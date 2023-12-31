use super::ipam::Ipam;

#[derive(Clone)]
pub struct AppState {
    pub ipam: Ipam,
}
