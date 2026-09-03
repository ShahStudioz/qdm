
use librqbit::{AddTorrent, AddTorrentOptions, Session};
use std::path::PathBuf;

#[tokio::test]
async fn test_magnet_behavior() -> Result<(), Box<dyn std::error::Error>> {
    let session = Session::new("test_downloads").await?;
    let magnet = "magnet:?xt=urn:btih:47e17f252a990ccaacff698698e5797950f31884&dn=%5BJudas%5D%20Kono%20Subarashii%20Sekai%20ni%20Bakuen%20wo%21%20%28KonoSuba%3A%20An%20Explosion%20on%20this%20Wonderful%20World%21%29%20%28Season%201%29%20%5B1080p%5D%5BHEVC%20x265%2010bit%5D%5BDual-Audio%5D%5BMulti-Subs%20%28Batch%29&tr=http%3A%2F%2Fnyaa.tracker.wf%3A7777%2Fannounce&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce&tr=udp%3A%2F%2Fexodus.desync.com%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.torrent.eu.org%3A451%2Fannounce";
    
    let opts = AddTorrentOptions {
        output_folder: Some("test_downloads".to_string()),
        overwrite: true,
        ..Default::default()
    };

    println!("Adding torrent...");
    let response = session.add_torrent(AddTorrent::Url(magnet.into()), Some(opts)).await?;
    
    match response {
        librqbit::AddTorrentResponse::Added(_, handle) => {
            println!("Torrent added. Waiting for metadata...");
            handle.wait_until_initialized().await?;
            let info = handle.shared_state().info.clone();
            println!("Info name: {:?}", info.name);
            println!("Is single file: {}", info.files.len() == 1);
            for f in info.files.iter().take(3) {
                println!("File: {:?}", f.path);
            }
        }
        _ => println!("Unexpected response"),
    }
    
    Ok(())
}

