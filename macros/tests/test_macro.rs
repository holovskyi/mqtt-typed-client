use mqtt_typed_client_macros::mqtt_topic_subscriber;

#[mqtt_topic_subscriber("test/topic/{huy}")] 
struct TestStruct {
    huy: u32,
    payload: String,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_macro_compiles() {

    }
} 