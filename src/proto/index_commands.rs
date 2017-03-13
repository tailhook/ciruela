use proto::NotificationTrait;

#[derive(Serialize, Deserialize, Debug)]
pub struct PublishIndex {
    pub image_id: Vec<u8>,
}

impl NotificationTrait for PublishIndex {
    fn type_name(&self) -> &'static str {
        "PublishIndex"
    }
}
