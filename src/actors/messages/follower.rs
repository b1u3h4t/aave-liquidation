use actix::prelude::*;
use alloy::providers::Provider;

use crate::actors::Fanatic;

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct StartListeningForOraclePrices;

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct StartListeningForEvents;

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct SendFanaticAddr<P: Provider + Unpin + Clone + 'static>(pub Addr<Fanatic<P>>);
