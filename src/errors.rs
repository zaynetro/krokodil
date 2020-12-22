#[derive(Debug)]
pub struct MissingGame;

impl warp::reject::Reject for MissingGame {}
