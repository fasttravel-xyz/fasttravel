use crate::*;

// type aliases till we push the 0.0.1-dev.0-services branch.
pub type ServiceCore = Mocker;
pub type ServiceIdentity = Mocker;
pub type ServicePresence = Mocker;
pub type ServiceActivity = Mocker;
pub type ServiceModel = Mocker;

// Mocking service used for testing.
pub struct Mocker {
    dispatcher: ExecutionContextObj,
}

impl Service for Mocker {
    fn new(dispatcher: ExecutionContextObj) -> Self {
        Mocker { dispatcher }
    }

    fn recv_connect(&mut self, client: ClientId) {
        println!("Mocker_recv_connect | client_id: {}", client.id);
    }

    fn recv_disconnect(&mut self, client: ClientId) {
        println!("Mocker_recv_disconnect | client_id: {}", client.id);
    }

    fn recv_text(&mut self, client: ClientId, text: &str) {
        println!(
            "Mocker_recv_text | client_id: {} | text: {}",
            client.id, text
        );

        let recipient = ServiceMessageRecipient::Client(client);
        self.dispatcher
            .tell_text(recipient, "MOCKER_REPLY_TO_CLIENT")
    }

    fn recv_encoded(&mut self, client: ClientId, _bytes: &ProtoBytes) {
        println!("Mocker_recv_binary | client_id: {}", client.id);
    }

    fn answer_encoded(&mut self, client: ClientId, _bytes: &ProtoBytes) -> ProtoResponse {
        println!("Mocker_answer_encoded | client_id: {}", client.id);
        Box::pin(async { Err(ProtoResponseError) })
    }
}
