use kervesh_ssh::{copy_stream, remote_join};
use tokio_util::sync::CancellationToken;
#[tokio::test]
async fn streaming_copy_preserves_bytes_and_reports_progress() {
    let data = vec![42u8; 200_000];
    let mut source = &data[..];
    let mut target = Vec::new();
    let mut progress = 0;
    let count = copy_stream(&mut source, &mut target, &CancellationToken::new(), |n| {
        progress = n
    })
    .await
    .unwrap();
    assert_eq!(count, 200_000);
    assert_eq!(target, data);
    assert_eq!(progress, 200_000);
}
#[tokio::test]
async fn cancelled_stream_does_not_write_destination() {
    let mut source = &b"important"[..];
    let mut target = Vec::new();
    let token = CancellationToken::new();
    token.cancel();
    assert!(
        copy_stream(&mut source, &mut target, &token, |_| {})
            .await
            .is_err()
    );
    assert!(target.is_empty());
}
#[test]
fn remote_paths_do_not_allow_sibling_traversal() {
    assert_eq!(
        remote_join("/var/www", "app.txt").unwrap(),
        "/var/www/app.txt"
    );
    for bad in ["../passwd", "/etc/passwd", "a/b", ".", "..", "bad\0name"] {
        assert!(remote_join("/var", bad).is_err());
    }
}
