//! Facilities to construct Consensus objects.
//!
//! (These are only for testing right now, since we don't yet
//! support signing or encoding.)

use super::{
    ConsensusFlavor, ConsensusVoterInfo, DirSource,
    Footer, Lifetime, NetParams, ProtoStatus, ProtoStatuses, SharedRandStatus,
    SharedRandVal,
};

use crate::{BuildError as Error, BuildResult as Result};
use tor_llcrypto::pk::rsa::RsaIdentity;
use tor_protover::Protocols;

use std::net::IpAddr;
use std::sync::Arc;
use std::time::SystemTime;

#[cfg(feature = "plain-consensus")]
pub(crate) mod plain;
pub(crate) mod md;

ns_export_each_variety! {
    ty: ConsensusBuilder;
}
