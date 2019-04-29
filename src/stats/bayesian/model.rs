use std::cmp::Ord;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use itertools::Itertools;
use ordered_float::NotNan;

use stats::LogProb;

pub type JointProbUniverse<Event> = BTreeMap<Event, LogProb>;

/// Likelihood model.
pub trait Likelihood<Payload = ()>
where
    Payload: Default,
{
    type Event;
    type Data;

    /// Compute likelihood of event given the data. Optionally, the passed payload can be used
    /// to e.g., cache intermediate results. One payload corresponds to one model instance.
    fn compute(&self, event: &Self::Event, data: &Self::Data, payload: &mut Payload) -> LogProb;
}

/// Prior model.
pub trait Prior {
    type Event;

    fn compute(&self, event: &Self::Event) -> LogProb;
}

/// Posterior model.
pub trait Posterior {
    type Event;
    type BaseEvent;
    type Data;

    fn compute<F: FnMut(&Self::BaseEvent, &Self::Data) -> LogProb>(
        &self,
        event: &Self::Event,
        data: &Self::Data,
        joint_prob: &mut F,
    ) -> LogProb;
}

pub struct Model<L, Pr, Po, Payload = ()>
where
    L: Likelihood<Payload>,
    Pr: Prior,
    Po: Posterior,
    Payload: Default,
{
    likelihood: L,
    prior: Pr,
    posterior: Po,
    payload: PhantomData<Payload>,
}

impl<Event, PosteriorEvent, Data, L, Pr, Po, Payload> Model<L, Pr, Po, Payload>
where
    Payload: Default,
    Event: Ord + Clone,
    PosteriorEvent: Ord + Clone,
    L: Likelihood<Payload, Event = Event, Data = Data>,
    Pr: Prior<Event = Event>,
    Po: Posterior<BaseEvent = Event, Event = PosteriorEvent, Data = Data>,
{
    pub fn new(likelihood: L, prior: Pr, posterior: Po) -> Self {
        Model {
            likelihood,
            prior,
            posterior,
            payload: PhantomData,
        }
    }

    pub fn joint_prob(&self, event: &Event, data: &Data, payload: &mut Payload) -> LogProb {
        self.prior.compute(event) + self.likelihood.compute(event, data, payload)
    }

    pub fn compute<U: IntoIterator<Item = PosteriorEvent>>(
        &self,
        universe: U,
        data: &Data,
    ) -> ModelInstance<Event, PosteriorEvent> {
        let mut joint_probs = BTreeMap::new();
        let mut payload = Payload::default();
        let (posterior_probs, marginal) = {
            let mut joint_prob = |event: &Event, data: &Data| {
                let p = self.joint_prob(event, data, &mut payload);
                joint_probs.insert(event.clone(), p);
                p
            };

            let posterior_probs: BTreeMap<PosteriorEvent, LogProb> = universe
                .into_iter()
                .map(|event| {
                    let p = self.posterior.compute(&event, data, &mut joint_prob);
                    (event, p)
                })
                .collect();
            let marginal = LogProb::ln_sum_exp(&posterior_probs.values().cloned().collect_vec());

            (posterior_probs, marginal)
        };

        ModelInstance {
            joint_probs,
            posterior_probs,
            marginal,
        }
    }
}

/// Instance of a model for given data and event universe.
/// From the instance, posterior, marginal and MAP can be computed.
pub struct ModelInstance<Event, PosteriorEvent>
where
    Event: Ord,
    PosteriorEvent: Ord,
{
    joint_probs: BTreeMap<Event, LogProb>,
    posterior_probs: BTreeMap<PosteriorEvent, LogProb>,
    marginal: LogProb,
}

impl<Event, PosteriorEvent> ModelInstance<Event, PosteriorEvent>
where
    Event: Ord,
    PosteriorEvent: Ord,
{
    /// Posterior probability of given event.
    pub fn posterior(&self, event: &PosteriorEvent) -> Option<LogProb> {
        self.posterior_probs.get(event).map(|p| p - self.marginal)
    }

    /// Marginal probability.
    pub fn marginal(&self) -> LogProb {
        self.marginal
    }

    /// Maximum a posteriori estimate.
    pub fn maximum_posterior(&self) -> Option<&Event> {
        self.joint_probs
            .iter()
            .max_by_key(|(_, prob)| NotNan::new(***prob).unwrap())
            .map(|(event, _)| event)
    }
}

impl<PosteriorEvent> ModelInstance<NotNan<f64>, PosteriorEvent>
where
    PosteriorEvent: Ord,
{
    pub fn expected_value(&self) -> NotNan<f64> {
        self.joint_probs
            .iter()
            .map(|(event, prob)| *event * NotNan::new(**prob).unwrap())
            .fold(NotNan::default(), |s, e| s + e)
    }
}

mod tests {}
