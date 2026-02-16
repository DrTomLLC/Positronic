//! Regression test: verify async channel send/receive works correctly.
//! Originally tested iced::futures mpsc; now tests tokio mpsc (post-iced migration).

#[tokio::test]
async fn channel_send_receive_works() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<u8>(1);

    tx.send(42).await.expect("send should succeed");

    let got = rx.recv().await.expect("should receive value");
    assert_eq!(got, 42);
}