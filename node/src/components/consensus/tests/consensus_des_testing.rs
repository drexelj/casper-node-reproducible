use super::queue::{MessageT, Queue, QueueEntry};
use crate::types::Timestamp;
use anyhow::anyhow;
use rand::Rng;
use std::cmp::Ordering;
use std::{
    collections::{BTreeMap, BinaryHeap, VecDeque},
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    time,
};

/// Enum defining recipients of the message.
pub(crate) enum Target {
    SingleValidator(ValidatorId),
    All,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct Message<M: Clone + Debug> {
    pub(crate) sender: ValidatorId,
    pub(crate) payload: M,
}

impl<M: Clone + Debug> Message<M> {
    pub(crate) fn new(sender: ValidatorId, payload: M) -> Self {
        Message { sender, payload }
    }

    pub(crate) fn payload(&self) -> &M {
        &self.payload
    }
}

pub(crate) struct TargetedMessage<M: Clone + Debug> {
    pub(crate) message: Message<M>,
    pub(crate) target: Target,
}

impl<M: Clone + Debug> TargetedMessage<M> {
    pub(crate) fn new(message: Message<M>, target: Target) -> Self {
        TargetedMessage { message, target }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub(crate) struct ValidatorId(pub(crate) u64);

/// A validator in the test network.
pub(crate) struct Validator<C, M, D>
where
    M: Clone + Debug,
{
    pub(crate) id: ValidatorId,
    /// Whether a validator should produce equivocations.
    pub(crate) is_faulty: bool,
    /// Vector of consensus values finalized by the validator.
    finalized_values: Vec<C>,
    /// Messages received by the validator.
    messages_received: Vec<Message<M>>,
    /// Messages produced by the validator.
    messages_produced: Vec<M>,
    /// An instance of consensus protocol.
    pub(crate) consensus: D,
}

impl<C, M, D> Validator<C, M, D>
where
    M: Clone + Debug,
{
    pub(crate) fn new(id: ValidatorId, is_faulty: bool, consensus: D) -> Self {
        Validator {
            id,
            is_faulty,
            finalized_values: Vec::new(),
            messages_received: Vec::new(),
            messages_produced: Vec::new(),
            consensus,
        }
    }

    pub(crate) fn is_faulty(&self) -> bool {
        self.is_faulty
    }

    pub(crate) fn validator_id(&self) -> ValidatorId {
        self.id
    }

    /// Adds vector of finalized consensus values to validator's finalized set.
    pub(crate) fn push_finalized(&mut self, finalized_values: Vec<C>) {
        self.finalized_values.extend(finalized_values);
    }

    /// Adds messages to validator's collection of received messages.
    pub(crate) fn push_messages_received(&mut self, messages: Vec<Message<M>>) {
        self.messages_received.extend(messages);
    }

    /// Adds messages to validator's collection of produced messages.
    pub(crate) fn push_messages_produced(&mut self, messages: Vec<M>) {
        self.messages_produced.extend(messages);
    }

    /// Iterator over consensus values finalized by the validator.
    pub(crate) fn finalized_values(&self) -> impl Iterator<Item = &C> {
        self.finalized_values.iter()
    }

    pub(crate) fn messages_received(&self) -> impl Iterator<Item = &Message<M>> {
        self.messages_received.iter()
    }

    pub(crate) fn messages_produced(&self) -> impl Iterator<Item = &M> {
        self.messages_produced.iter()
    }

    pub(crate) fn finalized_count(&self) -> usize {
        self.finalized_values.len()
    }
}

/// A trait defining strategy for randomly changing value of `i`.
///
/// Can be used to simulate network delays, message drops, invalid signatures,
/// panoramas etc.
pub(crate) trait Strategy<Item> {
    fn map<R: Rng>(&self, rng: &mut R, i: Item) -> Item {
        i
    }
}

pub(crate) enum DeliverySchedule {
    AtInstant(Timestamp),
    Drop,
}

impl DeliverySchedule {
    fn at(instant: Timestamp) -> DeliverySchedule {
        DeliverySchedule::AtInstant(instant)
    }

    fn drop(_instant: Timestamp) -> DeliverySchedule {
        DeliverySchedule::Drop
    }
}

impl From<u64> for DeliverySchedule {
    fn from(instant: u64) -> Self {
        DeliverySchedule::at(instant.into())
    }
}

impl From<Timestamp> for DeliverySchedule {
    fn from(timestamp: Timestamp) -> Self {
        DeliverySchedule::at(timestamp)
    }
}

pub(crate) struct VirtualNet<C, D, M, DS>
where
    M: MessageT,
    DS: Strategy<DeliverySchedule>,
{
    /// Maps validator IDs to actual validator instances.
    validators_map: BTreeMap<ValidatorId, Validator<C, M, D>>,
    /// A collection of all network messages queued up for delivery.
    msg_queue: Queue<M>,
    /// A strategy to pseudo randomly change the message delivery times.
    delivery_time_strategy: DS,
}

impl<C, D, M, DS> VirtualNet<C, D, M, DS>
where
    M: MessageT,
    DS: Strategy<DeliverySchedule>,
{
    pub(crate) fn new<I: IntoIterator<Item = Validator<C, M, D>>>(
        validators: I,
        delivery_time_strategy: DS,
        init_messages: Vec<QueueEntry<M>>,
    ) -> Self {
        let validators_map = validators
            .into_iter()
            .map(|validator| (validator.id, validator))
            .collect();

        let mut q = Queue::default();
        for m in init_messages.into_iter() {
            q.push(m);
        }

        VirtualNet {
            validators_map,
            msg_queue: q,
            delivery_time_strategy,
        }
    }

    /// Dispatches messages to their recipients.
    pub(crate) fn dispatch_messages<R: Rng>(
        &mut self,
        rand: &mut R,
        delivery_time: Timestamp,
        messages: Vec<TargetedMessage<M>>,
    ) {
        for TargetedMessage { message, target } in messages {
            let recipients = match target {
                Target::All => self.validators_ids().cloned().collect(),
                Target::SingleValidator(recipient_id) => vec![recipient_id],
            };
            self.send_messages(rand, recipients, message, delivery_time)
        }
    }

    /// Pop a message from the queue.
    /// It's a message with the earliest delivery time.
    pub(crate) fn pop_message(&mut self) -> Option<QueueEntry<M>> {
        self.msg_queue.pop()
    }

    pub(crate) fn get_validator(&self, validator: ValidatorId) -> Option<&Validator<C, M, D>> {
        self.validators_map.get(&validator)
    }

    pub(crate) fn validators_ids(&self) -> impl Iterator<Item = &ValidatorId> {
        self.validators_map.keys()
    }

    pub(crate) fn get_validator_mut(
        &mut self,
        validator_id: &ValidatorId,
    ) -> Option<&mut Validator<C, M, D>> {
        self.validators_map.get_mut(validator_id)
    }

    pub(crate) fn validator(&self, validator_id: &ValidatorId) -> Option<&Validator<C, M, D>> {
        self.validators_map.get(validator_id)
    }

    pub(crate) fn validators(&self) -> impl Iterator<Item = &Validator<C, M, D>> {
        self.validators_map.values()
    }

    // Utility function for dispatching message to multiple recipients.
    fn send_messages<R: Rng, I: IntoIterator<Item = ValidatorId>>(
        &mut self,
        rand: &mut R,
        recipients: I,
        message: Message<M>,
        base_delivery_time: Timestamp,
    ) {
        for validator_id in recipients {
            let tampered_delivery_time = self
                .delivery_time_strategy
                .map(rand, base_delivery_time.into());
            match tampered_delivery_time {
                // Simulates dropping of the message.
                // TODO: Add logging.
                DeliverySchedule::Drop => (),
                DeliverySchedule::AtInstant(dt) => {
                    self.schedule_message(dt, validator_id, message.clone())
                }
            }
        }
    }

    /// Schedules a message `message` to be delivered at `delivery_time` to `recipient` validator.
    fn schedule_message(
        &mut self,
        delivery_time: Timestamp,
        recipient: ValidatorId,
        message: Message<M>,
    ) {
        let qe = QueueEntry::new(delivery_time, recipient, message);
        self.msg_queue.push(qe);
    }

    /// Drops all messages from the queue.
    /// Should never be called during normal operation of the test.
    pub(crate) fn empty_queue(&mut self) {
        self.msg_queue.clear();
    }
}

mod virtual_net_tests {

    use super::{
        DeliverySchedule, Message, Strategy, Target, TargetedMessage, Timestamp, Validator,
        ValidatorId, VirtualNet,
    };
    use rand_core::SeedableRng;
    use rand_xorshift::XorShiftRng;
    use std::collections::{HashSet, VecDeque};

    struct NoOpDelay;

    impl Strategy<DeliverySchedule> for NoOpDelay {
        fn map<R: rand::Rng>(&self, _rng: &mut R, i: DeliverySchedule) -> DeliverySchedule {
            i
        }
    }

    type M = u64;
    type C = u64;

    struct NoOpConsensus;

    #[test]
    fn messages_are_enqueued_in_order() {
        let validator_id = ValidatorId(1u64);
        let single_validator: Validator<C, u64, NoOpConsensus> =
            Validator::new(validator_id, false, NoOpConsensus);
        let mut virtual_net = VirtualNet::new(vec![single_validator], NoOpDelay, vec![]);

        let messages_num = 10;
        // We want to enqueue messages from the latest delivery time to the earliest.
        let messages: Vec<(Timestamp, Message<u64>)> = (0..messages_num)
            .map(|i| ((messages_num - i).into(), Message::new(validator_id, i)))
            .collect();

        messages.clone().into_iter().for_each(|(instant, message)| {
            virtual_net.schedule_message(instant, validator_id, message)
        });

        let queued_messages =
            std::iter::successors(virtual_net.pop_message(), |_| virtual_net.pop_message())
                .map(|qe| qe.message);

        // Since we enqueued in the order from the latest delivery time,
        // we expect that the actual delivery will be a reverse.
        let expected_order = messages.into_iter().map(|(_, msg)| msg).rev();

        assert!(
            queued_messages.eq(expected_order),
            "Messages were not delivered in the expected order."
        );
    }

    #[test]
    fn messages_are_dispatched() {
        let validator_id = ValidatorId(1u64);
        let first_validator: Validator<C, M, NoOpConsensus> =
            Validator::new(validator_id, false, NoOpConsensus);
        let second_validator: Validator<C, M, NoOpConsensus> =
            Validator::new(ValidatorId(2u64), false, NoOpConsensus);

        let mut virtual_net =
            VirtualNet::new(vec![first_validator, second_validator], NoOpDelay, vec![]);
        let mut rand = XorShiftRng::from_seed(rand::random());

        let message = Message::new(validator_id, 1u64);
        let targeted_message = TargetedMessage::new(message.clone(), Target::All);

        virtual_net.dispatch_messages(&mut rand, 2.into(), vec![targeted_message]);

        let queued_msgs =
            std::iter::successors(virtual_net.pop_message(), |_| virtual_net.pop_message())
                .map(|qe| (qe.recipient, qe.message))
                .collect::<Vec<_>>();

        assert_eq!(
            queued_msgs,
            vec![(ValidatorId(2), message.clone()), (ValidatorId(1), message)],
            "A broadcast message should be delivered to every node."
        );
    }
}
