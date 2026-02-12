use iced::futures::{SinkExt, StreamExt, channel::mpsc};

#[tokio::test]
async fn sinkext_send_is_available_for_iced_mpsc_sender() {
    let (mut tx, mut rx) = mpsc::channel::<u8>(1);

    tx.send(42).await.expect("send should succeed");

    let got = rx.next().await.expect("should receive value");
    assert_eq!(got, 42);
}
