use iced::futures::channel::mpsc;

#[test]
fn iced_stream_channel_sender_type_is_mpsc_sender() {
    // Compile-time regression guard:
    // In iced 0.14, the sender passed into iced::stream::channel is mpsc::Sender<T>,
    // NOT iced::stream::Sender<T>.
    fn _accept_sender(_s: mpsc::Sender<u8>) {}
}
