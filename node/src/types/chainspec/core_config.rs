use datasize::DataSize;
use num::rational::Ratio;
#[cfg(test)]
use rand::Rng;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use casper_types::testing::TestRng;
use casper_types::{
    bytesrepr::{self, FromBytes, ToBytes},
    TimeDiff,
};

#[derive(Copy, Clone, DataSize, PartialEq, Eq, Serialize, Deserialize, Debug)]
// Disallow unknown fields to ensure config files and command-line overrides contain valid keys.
#[serde(deny_unknown_fields)]
pub struct CoreConfig {
    pub(crate) era_duration: TimeDiff,
    pub(crate) minimum_era_height: u64,
    pub(crate) validator_slots: u32,
    /// Number of eras before an auction actually defines the set of validators.
    /// If you bond with a sufficient bid in era N, you will be a validator in era N +
    /// auction_delay + 1
    pub(crate) auction_delay: u64,
    /// The period after genesis during which a genesis validator's bid is locked.
    pub(crate) locked_funds_period: TimeDiff,
    /// The period in which genesis validator's bid is released over time after it's unlocked.
    pub(crate) vesting_schedule_period: TimeDiff,
    /// The delay in number of eras for paying out the the unbonding amount.
    pub(crate) unbonding_delay: u64,
    /// Round seigniorage rate represented as a fractional number.
    #[data_size(skip)]
    pub(crate) round_seigniorage_rate: Ratio<u64>,
    /// Maximum number of associated keys for a single account.
    pub(crate) max_associated_keys: u32,
    /// Maximum height of contract runtime call stack.
    pub(crate) max_runtime_call_stack_height: u32,
    /// The minimum bound of motes that can be delegated to a validator.
    pub(crate) minimum_delegation_amount: u64,
    /// Enables strict arguments checking when calling a contract.
    pub(crate) strict_argument_checking: bool,
}

#[cfg(test)]
impl CoreConfig {
    /// Generates a random instance using a `TestRng`.
    pub fn random(rng: &mut TestRng) -> Self {
        let era_duration = TimeDiff::from(rng.gen_range(600_000..604_800_000));
        let minimum_era_height = rng.gen_range(5..100);
        let validator_slots = rng.gen();
        let auction_delay = rng.gen::<u32>() as u64;
        let locked_funds_period = TimeDiff::from(rng.gen_range(600_000..604_800_000));
        let vesting_schedule_period = TimeDiff::from(rng.gen_range(600_000..604_800_000));
        let unbonding_delay = rng.gen_range(1..1_000_000_000);
        let round_seigniorage_rate = Ratio::new(
            rng.gen_range(1..1_000_000_000),
            rng.gen_range(1..1_000_000_000),
        );
        let max_associated_keys = rng.gen();
        let max_runtime_call_stack_height = rng.gen();
        let minimum_delegation_amount = rng.gen::<u32>() as u64;
        let strict_argument_checking = rng.gen();

        CoreConfig {
            era_duration,
            minimum_era_height,
            validator_slots,
            auction_delay,
            locked_funds_period,
            vesting_schedule_period,
            unbonding_delay,
            round_seigniorage_rate,
            max_associated_keys,
            max_runtime_call_stack_height,
            minimum_delegation_amount,
            strict_argument_checking,
        }
    }
}

impl ToBytes for CoreConfig {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        buffer.extend(self.era_duration.to_bytes()?);
        buffer.extend(self.minimum_era_height.to_bytes()?);
        buffer.extend(self.validator_slots.to_bytes()?);
        buffer.extend(self.auction_delay.to_bytes()?);
        buffer.extend(self.locked_funds_period.to_bytes()?);
        buffer.extend(self.vesting_schedule_period.to_bytes()?);
        buffer.extend(self.unbonding_delay.to_bytes()?);
        buffer.extend(self.round_seigniorage_rate.to_bytes()?);
        buffer.extend(self.max_associated_keys.to_bytes()?);
        buffer.extend(self.max_runtime_call_stack_height.to_bytes()?);
        buffer.extend(self.minimum_delegation_amount.to_bytes()?);
        buffer.extend(self.strict_argument_checking.to_bytes()?);
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.era_duration.serialized_length()
            + self.minimum_era_height.serialized_length()
            + self.validator_slots.serialized_length()
            + self.auction_delay.serialized_length()
            + self.locked_funds_period.serialized_length()
            + self.vesting_schedule_period.serialized_length()
            + self.unbonding_delay.serialized_length()
            + self.round_seigniorage_rate.serialized_length()
            + self.max_associated_keys.serialized_length()
            + self.max_runtime_call_stack_height.serialized_length()
            + self.minimum_delegation_amount.serialized_length()
            + self.strict_argument_checking.serialized_length()
    }
}

impl FromBytes for CoreConfig {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (era_duration, remainder) = TimeDiff::from_bytes(bytes)?;
        let (minimum_era_height, remainder) = u64::from_bytes(remainder)?;
        let (validator_slots, remainder) = u32::from_bytes(remainder)?;
        let (auction_delay, remainder) = u64::from_bytes(remainder)?;
        let (locked_funds_period, remainder) = TimeDiff::from_bytes(remainder)?;
        let (vesting_schedule_period, remainder) = TimeDiff::from_bytes(remainder)?;
        let (unbonding_delay, remainder) = u64::from_bytes(remainder)?;
        let (round_seigniorage_rate, remainder) = Ratio::<u64>::from_bytes(remainder)?;
        let (max_associated_keys, remainder) = u32::from_bytes(remainder)?;
        let (max_runtime_call_stack_height, remainder) = u32::from_bytes(remainder)?;
        let (minimum_delegation_amount, remainder) = u64::from_bytes(remainder)?;
        let (strict_argument_checking, remainder) = bool::from_bytes(remainder)?;
        let config = CoreConfig {
            era_duration,
            minimum_era_height,
            validator_slots,
            auction_delay,
            locked_funds_period,
            vesting_schedule_period,
            unbonding_delay,
            round_seigniorage_rate,
            max_associated_keys,
            max_runtime_call_stack_height,
            minimum_delegation_amount,
            strict_argument_checking,
        };
        Ok((config, remainder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytesrepr_roundtrip() {
        let mut rng = crate::new_rng();
        let config = CoreConfig::random(&mut rng);
        bytesrepr::test_serialization_roundtrip(&config);
    }

    #[test]
    fn toml_roundtrip() {
        let mut rng = crate::new_rng();
        let config = CoreConfig::random(&mut rng);
        let encoded = toml::to_string_pretty(&config).unwrap();
        let decoded = toml::from_str(&encoded).unwrap();
        assert_eq!(config, decoded);
    }
}
