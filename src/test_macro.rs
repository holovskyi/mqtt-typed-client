use crate::mqtt_topic_subscriber;

#[mqtt_topic_subscriber("test/topic")]
struct TestStruct {
    field: String,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_macro_compiles() {

    }
}