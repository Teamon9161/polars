use super::*;
use crate::mmap::MmapBytesReader;
use crate::utils::apply_projection;
use arrow::chunk::Chunk;
use arrow::io::ipc::read;
use arrow::io::ipc::read::{Dictionaries, FileMetadata};
use arrow::mmap::{mmap_dictionaries_unchecked, mmap_unchecked};
use memmap::Mmap;

struct MMapChunkIter<'a> {
    dictionaries: Dictionaries,
    metadata: FileMetadata,
    mmap: Arc<Mmap>,
    idx: usize,
    end: usize,
    projection: &'a Option<Vec<usize>>,
}

impl<'a> MMapChunkIter<'a> {
    fn new(mmap: Mmap, metadata: FileMetadata, projection: &'a Option<Vec<usize>>) -> Result<Self> {
        let mmap = Arc::new(mmap);

        let end = metadata.blocks.len();
        // mmap the dictionaries
        let dictionaries = unsafe { mmap_dictionaries_unchecked(&metadata, mmap.clone())? };

        Ok(Self {
            dictionaries,
            metadata,
            mmap,
            idx: 0,
            end,
            projection,
        })
    }
}

impl ArrowReader for MMapChunkIter<'_> {
    fn next_record_batch(&mut self) -> ArrowResult<Option<ArrowChunk>> {
        if self.idx < self.end {
            let chunk = unsafe {
                mmap_unchecked(
                    &self.metadata,
                    &self.dictionaries,
                    self.mmap.clone(),
                    self.idx,
                )
            }?;
            self.idx += 1;
            let chunk = match &self.projection {
                None => chunk,
                Some(proj) => {
                    let cols = chunk.into_arrays();
                    let arrays = proj.iter().map(|i| cols[*i].clone()).collect();
                    Chunk::new(arrays)
                }
            };
            Ok(Some(chunk))
        } else {
            Ok(None)
        }
    }
}

impl<R: MmapBytesReader> IpcReader<R> {
    pub(super) fn finish_memmapped(
        &self,
        predicate: Option<Arc<dyn PhysicalIoExpr>>,
        aggregate: Option<&[ScanAggregation]>,
    ) -> Result<DataFrame> {
        match self.reader.to_file() {
            Some(file) => {
                let mmap = unsafe { memmap::Mmap::map(file).unwrap() };
                let metadata = read::read_file_metadata(&mut std::io::Cursor::new(mmap.as_ref()))?;

                let schema = if let Some(projection) = &self.projection {
                    apply_projection(&metadata.schema, projection)
                } else {
                    metadata.schema.clone()
                };

                let reader = MMapChunkIter::new(mmap, metadata, &self.projection)?;

                finish_reader(
                    reader,
                    // don't rechunk, that would trigger a read.
                    false,
                    self.n_rows,
                    predicate,
                    aggregate,
                    &schema,
                    self.row_count.clone(),
                )
            }
            None => Err(PolarsError::ComputeError(
                "Cannot memory map, you must provide a file".into(),
            )),
        }
    }
}
