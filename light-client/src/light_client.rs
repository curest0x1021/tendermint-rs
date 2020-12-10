//! Light client implementation as per the [Core Verification specification][1].
//!
//! [1]: https://github.com/informalsystems/tendermint-rs/blob/master/docs/spec/lightclient/verification/verification.md

use contracts::*;
use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

use crate::components::{clock::Clock, io::*, scheduler::*, verifier::*};
use crate::contracts::*;
use crate::{
    bail,
    errors::{Error, ErrorKind},
    state::State,
    types::{Height, LightBlock, PeerId, Status, TrustThreshold},
};

/// Verification parameters
///
/// TODO: Find a better name than `Options`
#[derive(Copy, Clone, Debug, PartialEq, Display, Serialize, Deserialize)]
#[display(fmt = "{:?}", self)]
pub struct Options {
    /// Defines what fraction of the total voting power of a known
    /// and trusted validator set is sufficient for a commit to be
    /// accepted going forward.
    pub trust_threshold: TrustThreshold,

    /// How long a validator set is trusted for (must be shorter than the chain's
    /// unbonding period)
    pub trusting_period: Duration,

    /// Correction parameter dealing with only approximately synchronized clocks.
    /// The local clock should always be ahead of timestamps from the blockchain; this
    /// is the maximum amount that the local clock may drift behind a timestamp from the
    /// blockchain.
    pub clock_drift: Duration,
}

/// The light client implements a read operation of a header from the blockchain,
/// by communicating with full nodes. As full nodes may be faulty, it cannot trust
/// the received information, but the light client has to check whether the header
/// it receives coincides with the one generated by Tendermint consensus.
///
/// In the Tendermint blockchain, the validator set may change with every new block.
/// The staking and unbonding mechanism induces a security model: starting at time
/// of the header, more than two-thirds of the next validators of a new block are
/// correct for the duration of the trusted period.  The fault-tolerant read operation
/// is designed for this security model.
pub struct LightClient {
    /// The peer id of the peer this client is connected to
    pub peer: PeerId,
    /// Options for this light client
    pub options: Options,
    clock: Box<dyn Clock>,
    scheduler: Box<dyn Scheduler>,
    verifier: Box<dyn Verifier>,
    io: Box<dyn Io>,
}

impl fmt::Debug for LightClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LightClient")
            .field("peer", &self.peer)
            .field("options", &self.options)
            .finish()
    }
}

impl LightClient {
    /// Constructs a new light client
    pub fn new(
        peer: PeerId,
        options: Options,
        clock: impl Clock + 'static,
        scheduler: impl Scheduler + 'static,
        verifier: impl Verifier + 'static,
        io: impl Io + 'static,
    ) -> Self {
        Self {
            peer,
            options,
            clock: Box::new(clock),
            scheduler: Box::new(scheduler),
            verifier: Box::new(verifier),
            io: Box::new(io),
        }
    }

    /// Constructs a new light client from boxed components
    pub fn from_boxed(
        peer: PeerId,
        options: Options,
        clock: Box<dyn Clock>,
        scheduler: Box<dyn Scheduler>,
        verifier: Box<dyn Verifier>,
        io: Box<dyn Io>,
    ) -> Self {
        Self {
            peer,
            options,
            clock,
            scheduler,
            verifier,
            io,
        }
    }

    /// Attempt to update the light client to the highest block of the primary node.
    ///
    /// Note: This function delegates the actual work to `verify_to_target`.
    pub fn verify_to_highest(&mut self, state: &mut State) -> Result<LightBlock, Error> {
        let target_block = match self.io.fetch_light_block(AtHeight::Highest) {
            Ok(last_block) => last_block,
            Err(io_error) => bail!(ErrorKind::Io(io_error)),
        };

        self.verify_to_target(target_block.height(), state)
    }

    /// Update the light client to a block of the primary node at the given height.
    ///
    /// This is the main function and uses the following components:
    ///
    /// - The I/O component is called to fetch the next light block. It is the only component that
    ///   communicates with other nodes.
    /// - The Verifier component checks whether a header is valid and checks if a new light block
    ///   should be trusted based on a previously verified light block.
    /// - The Scheduler component decides which height to try to verify next, in case the current
    ///   block pass verification but cannot be trusted yet.
    ///
    /// ## Implements
    /// - [LCV-DIST-SAFE.1]
    /// - [LCV-DIST-LIFE.1]
    /// - [LCV-PRE-TP.1]
    /// - [LCV-POST-LS.1]
    /// - [LCV-INV-TP.1]
    ///
    /// ## Postcondition
    /// - The light store contains a light block that corresponds to a block of the blockchain of
    ///   height `target_height` [LCV-POST-LS.1]
    ///
    /// ## Error conditions
    /// - The light store does not contains a trusted light block within the trusting period
    ///   [LCV-PRE-TP.1]
    /// - If the core verification loop invariant is violated [LCV-INV-TP.1]
    /// - If verification of a light block fails
    /// - If the fetching a light block from the primary node fails
    pub fn verify_to_target(
        &self,
        target_height: Height,
        state: &mut State,
    ) -> Result<LightBlock, Error> {
        let (light_block, _) = self.get_or_fetch_block(target_height, state)?;
        Ok(light_block)
    }

    /// Look in the light store for a block from the given peer at the given height,
    /// which has not previously failed verification (ie. its status is not `Failed`).
    ///
    /// If one cannot be found, fetch the block from the given peer and store
    /// it in the light store with `Unverified` status.
    ///
    /// ## Postcondition
    /// - The provider of block that is returned matches the given peer.
    #[post(ret.as_ref().map(|(lb, _)| lb.provider == self.peer).unwrap_or(true))]
    pub fn get_or_fetch_block(
        &self,
        height: Height,
        state: &mut State,
    ) -> Result<(LightBlock, Status), Error> {
        let block = state.light_store.get_non_failed(height);

        if let Some(block) = block {
            return Ok(block);
        }

        let block = self
            .io
            .fetch_light_block(AtHeight::At(height))
            .map_err(ErrorKind::Io)?;

        state.light_store.insert(block.clone(), Status::Unverified);

        Ok((block, Status::Unverified))
    }
}
