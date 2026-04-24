use soroban_sdk::{contracterror, contracttype, Bytes, String};

#[contracttype]
pub enum DataKey {
    Client,
    Freelancer,
    Milestones,
    Initialized,
    TermsHash,
    GracePeriod,
    MilestoneApprovalTime,
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
    GracePeriodNotExpired = 6,
    TermsHashAlreadySet = 7,
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
    pub approval_time: Option<u64>,
}

