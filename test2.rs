use librqbit::storage::{StorageFactory, TorrentStorage};
use librqbit::torrent_state::{ManagedTorrentShared, TorrentMetadata};

pub struct MyStorageFactory {}

impl StorageFactory for MyStorageFactory {
    type Storage = Box<dyn TorrentStorage>;

    fn create(
        &self,
        shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> anyhow::Result<Self::Storage> {
        unimplemented!()
    }
}
fn main() {}
