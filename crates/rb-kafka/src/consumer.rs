use std::{marker::PhantomData, sync::Arc, time::Duration};

use futures::StreamExt as _;
use prost::Message as ProstMessage;
use rdkafka::{
    consumer::{CommitMode, Consumer as RdConsumer, StreamConsumer},
    message::{Header, Headers as RdHeaders, Message as RdMessage},
    ClientConfig, TopicPartitionList,
};

use crate::{
    config::ConsumerCfg,
    dlq::dlq_topic,
    envelope::{EventEnvelope, HEADER_DLQ_AT, HEADER_DLQ_REASON},
    errors::KafkaError,
    producer::decode_envelope,
};

pub struct Consumer<E: ProstMessage + Default> {
    inner: Arc<StreamConsumer>,
    dlq_producer: Arc<rdkafka::producer::FutureProducer>,
    _phantom: PhantomData<E>,
}

#[allow(clippy::used_underscore_binding, clippy::unused_async)]
impl<E: ProstMessage + Default> Consumer<E> {
    pub fn new(cfg: &ConsumerCfg) -> Result<Self, KafkaError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.bootstrap_servers)
            .set("group.id", &cfg.group_id)
            .set("enable.auto.commit", cfg.enable_auto_commit.to_string())
            .set("isolation.level", &cfg.isolation_level)
            .set("auto.offset.reset", &cfg.auto_offset_reset)
            .set("max.poll.interval.ms", cfg.max_poll_interval.as_millis().to_string())
            .set("session.timeout.ms", cfg.session_timeout.as_millis().to_string())
            .create()?;

        let dlq_producer = ClientConfig::new()
            .set("bootstrap.servers", &cfg.bootstrap_servers)
            .set("acks", "1")
            .create()?;

        Ok(Self {
            inner: Arc::new(consumer),
            dlq_producer: Arc::new(dlq_producer),
            _phantom: PhantomData,
        })
    }

    pub fn subscribe(&self, topics: &[&str]) -> Result<(), KafkaError> {
        self.inner.subscribe(topics)?;
        Ok(())
    }

    pub async fn next(&self) -> Option<Result<EventEnvelope<E>, KafkaError>> {
        let msg = self.inner.stream().next().await?;
        match msg {
            Err(e) => Some(Err(KafkaError::from(e))),
            Ok(m) => {
                let payload = m.payload().unwrap_or(&[]);
                let headers = extract_headers(m.headers());
                let topic = m.topic().to_owned();
                let partition = m.partition();
                let offset = m.offset();
                Some(decode_envelope(payload, &headers, &topic, partition, offset))
            }
        }
    }

    pub async fn commit(&self, env: &EventEnvelope<E>) -> Result<(), KafkaError> {
        let mut tpl = TopicPartitionList::new();
        tpl.add_partition_offset(
            &env._meta.topic,
            env._meta.partition,
            rdkafka::Offset::Offset(env._meta.offset + 1),
        )?;
        self.inner.commit(&tpl, CommitMode::Async)?;
        Ok(())
    }

    pub async fn nack_to_dlq(
        &self,
        env: &EventEnvelope<E>,
        reason: &str,
    ) -> Result<(), KafkaError> {
        use rdkafka::producer::FutureRecord;

        let dlq = dlq_topic(&env._meta.topic);
        let dlq_at = chrono::Utc::now().timestamp_millis().to_string();
        let headers = rdkafka::message::OwnedHeaders::new()
            .insert(Header { key: HEADER_DLQ_REASON, value: Some(reason.as_bytes()) })
            .insert(Header { key: HEADER_DLQ_AT, value: Some(dlq_at.as_bytes()) });

        let payload = env.payload.encode_to_vec();
        let key = env.tenant_id.to_string();
        let record = FutureRecord::to(dlq.as_str())
            .key(key.as_bytes())
            .payload(payload.as_slice())
            .headers(headers);

        self.dlq_producer
            .send(record, Duration::from_secs(10))
            .await
            .map_err(|(e, _)| KafkaError::from(e))?;

        Ok(())
    }
}

fn extract_headers(h: Option<&rdkafka::message::BorrowedHeaders>) -> Vec<(String, String)> {
    let Some(bh) = h else { return Vec::new() };
    bh.iter()
        .filter_map(|header| {
            let key = header.key.to_owned();
            let value = std::str::from_utf8(header.value.unwrap_or(&[])).ok()?.to_owned();
            Some((key, value))
        })
        .collect()
}
