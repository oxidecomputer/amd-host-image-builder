use amd_efs::ProcessorGeneration;
use amd_flash::{ErasableRange, Location};
use amd_host_image_builder_config::{Error, Result};

pub struct Allocator {
    efh_range: ErasableRange,
    free_ranges: [ErasableRange; 2],
}

impl Allocator {
    /// Creates a new allocator that will use parts of the given ARENA.
    /// Depending on PROCESSOR_GENERATION, a part of it will be cut out
    /// and not given to the user (since it needs to be at a fixed
    /// spot and also is used by us).
    pub fn new(
        processor_generation: ProcessorGeneration,
        arena: ErasableRange,
    ) -> Result<Self> {
        let mut arena = arena;
        assert!(Location::from(arena.beginning) == 0);
        // Avoid EFH_BEGINNING..(EFH_BEGINNING + EFH_SIZE)
        let a_size =
            crate::static_config::EFH_BEGINNING(processor_generation) as usize;
        let a = arena.take_at_least(a_size).ok_or(Error::ImageTooBig)?;
        assert!(Location::from(a.end) as usize == a_size);
        let efh_range = arena
            .take_at_least(crate::static_config::EFH_SIZE)
            .ok_or(Error::ImageTooBig)?;
        Ok(Self { efh_range, free_ranges: [a, arena] })
    }
    /// From the free ranges, take a range of at least SIZE Bytes,
    /// if possible. Otherwise return None.
    pub fn take_at_least(&mut self, size: usize) -> Option<ErasableRange> {
        self.free_ranges[0]
            .take_at_least(size)
            .or_else(|| self.free_ranges[1].take_at_least(size))
    }
}

#[cfg(test)]
mod allocator_tests {
    use super::*;
    use amd_flash::{FlashAlign, Location};
    fn intersect(
        a: &ErasableRange,
        b: &ErasableRange,
    ) -> Option<(Location, Location)> {
        let new_beginning =
            Location::from(a.beginning).max(Location::from(b.beginning));
        let new_end = Location::from(a.end).min(Location::from(b.end));
        if new_beginning < new_end {
            Some((new_beginning, new_end))
        } else {
            None
        }
    }
    struct Buffer {}
    impl FlashAlign for Buffer {
        fn erasable_block_size(&self) -> usize {
            4
        }
    }
    impl Buffer {
        fn allocator(&self) -> Allocator {
            let beginning = self.erasable_location(0).unwrap();
            let end = beginning.advance_at_least(0x4_0000).unwrap();
            Allocator::new(
                ProcessorGeneration::Naples, // Note: Hole is at 0x2_0000.
                ErasableRange::new(beginning, end),
            )
            .unwrap()
        }
        fn efh_range(&self) -> ErasableRange {
            ErasableRange::new(
                self.erasable_location(0x2_0000).unwrap(),
                self.erasable_location(0x2_0000)
                    .unwrap()
                    .advance_at_least(crate::static_config::EFH_SIZE)
                    .unwrap(),
            )
        }
    }
    #[test]
    fn test_allocator_1() {
        let buf = Buffer {};
        let mut allocator = buf.allocator();
        let efh_range = buf.efh_range();
        let a = allocator.take_at_least(42).unwrap();
        let b = allocator.take_at_least(100).unwrap();
        assert!(intersect(&a, &b).is_none());
        assert!(intersect(&a, &efh_range).is_none());
        assert!(intersect(&b, &efh_range).is_none());
        assert!(Location::from(b.end) < 0x2_0000);
    }

    #[test]
    fn test_allocator_2() {
        let buf = Buffer {};
        let mut allocator = buf.allocator();
        let efh_range = buf.efh_range();
        let a = allocator.take_at_least(0x2_0000).unwrap();
        let b = allocator.take_at_least(100).unwrap();
        assert!(intersect(&a, &b).is_none());
        assert!(intersect(&a, &efh_range).is_none());
        assert!(intersect(&b, &efh_range).is_none());
        assert!(Location::from(b.end) < 0x4_0000);
    }

    #[test]
    fn test_allocator_3() {
        let buf = Buffer {};
        let mut allocator = buf.allocator();
        let efh_range = buf.efh_range();
        let a = allocator.take_at_least(0x1_fff8).unwrap();
        let b = allocator.take_at_least(100).unwrap();
        assert!(intersect(&a, &b).is_none());
        assert!(intersect(&a, &efh_range).is_none());
        assert!(intersect(&b, &efh_range).is_none());
        assert!(Location::from(b.end) < 0x4_0000);
        assert!(Location::from(b.beginning) > 0x2_0000);
    }
}
