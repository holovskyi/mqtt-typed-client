# Open Tasks

## Easy Tasks
- [ ] Auto-generation of client_id
- [ ] Add subscriber.topic() and .pattern() methods for `let mut subscriber = topic_client.subscribe().await?;`

## Medium Difficulty
- [ ] Default Bincode serializer
- [ ] Add retain, qos, dup flags to incoming message metadata. In async_client.rs/Ok(Incoming(Publish(p))) => we receive this data but discard it. The retain flag is particularly useful. Could be implemented through raw_data field in topic structure, similar to auto-populated field topic: Arc<TopicMatch>)
- [ ] Add default QoS level parameter to mqtt_topic macro

## Hard Tasks
- [ ] Add subscription acknowledgment. Currently subscribe doesn't analyze subscription results. Need to add event_loop mechanism that gets packet id from Outgoing(Subscribe(1)) for subscription, then waits for Incoming(SubAck(SubAck { pkid: 1, return_codes: [Success(AtLeastOnce)] })) with subscription result and provides this result to user. But there's a problem that currently Outgoing(Subscribe()) only has pkid without specific filters, so we need to fork rumqttc and change Outgoing::Subscribe(u16) => outgoing::Subscribe(Subscribe) in src/lib.rs.
- [ ] Add publish acknowledgment confirmation. Currently we don't know when the broker has actually confirmed message publication according to QoS level. This also requires forking rumqttc to track packet IDs and match them with corresponding ACK responses.

## Advanced Features
- [ ] Protocol compression. Compositional approach - separate serialization and compression. Adaptive mechanism (based on message type and size)
- [ ] mqtt_typed_client::client::async_client::MqttClient impl<F> MqttClient<F> async fn run(mut event_loop: EventLoop, subscription_manager: SubscriptionManagerHandler<Bytes>) - When we get a certain number of errors, we exit the loop. But maybe we should propagate errors to other levels, to subscribers and publishers?

## Optional/Future
- [ ] Add notypedclient and nolastwill options to macro

## Issues to Investigate
- [ ] Does the client see generated types like SensorReadingSubscriptionBuilderExt? Should we shorten the name?
- [ ] Problem with cryptic error when we declare a structure for payload but forget to add derive for custom serializer to work with the structure. For example, BincodeSerializer needs Encode and Decode. But we get an error where it's hard to determine what exactly is needed