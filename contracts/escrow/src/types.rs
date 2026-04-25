use soroban_sdk::{contracterror, contracttype, String};

#[contracttype]
pub enum DataKey {
    Client,
    Freelancer,
    Milestones,
    Initialized,
    ReadinessChecklist,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadinessChecklist {
    pub caps_set: bool,
    pub governed_params_set: bool,
    pub emergency_controls_enabled: bool,
    pub initialized: bool,
}

impl Default for ReadinessChecklist {
    fn default() -> Self {
        Self {
            caps_set: false,
            governed_params_set: false,
            emergency_controls_enabled: false,
            initialized: false,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MainnetReadinessInfo {
    pub caps_set: bool,
    pub governed_params_set: bool,
    pub emergency_controls_enabled: bool,
    pub initialized: bool,
    pub protocol_version: u32,
    pub max_escrow_total_stroops: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    IndexOutOfBounds = 3,
    AlreadyReleased = 4,
    InvalidStatusTransition = 5,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractStatus {
    Created = 0,
    Funded = 1,
    Completed = 2,
    Disputed = 3,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Milestone {
    pub amount: i128,
    pub released: bool,
    pub work_evidence: Option<String>,
}
