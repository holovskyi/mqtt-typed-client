//! Test validation of compatible patterns
//!
//! This example demonstrates pattern validation logic

use mqtt_typed_client::{TopicPatternPath, TopicPatternError, CacheStrategy};

fn test_pattern_validation() -> Result<(), TopicPatternError> {
    println!("Testing pattern validation logic...");
    
    // Create original pattern
    let original = TopicPatternPath::new_from_string(
        "sensors/{building}/{floor}/temp/{sensor_id}",
        CacheStrategy::NoCache
    )?;
    
    println!("Original pattern: {}", original.topic_pattern());
    println!("Original MQTT: {}", original.mqtt_pattern());
    
    // ✅ Test 1: Valid compatible pattern (different static segments)
    let compatible = original.check_pattern_compatibility(
        "data/{building}/{floor}/temperature/{sensor_id}",
    )?;
    
    println!("✅ Compatible pattern works: {}", compatible.topic_pattern());
    assert_eq!(compatible.mqtt_pattern(), "data/+/+/temperature/+");
    
    // ✅ Test 2: Another valid pattern
    let legacy = original.check_pattern_compatibility(
        "iot/{building}/{floor}/t/{sensor_id}",
    )?;
    
    println!("✅ Legacy pattern works: {}", legacy.topic_pattern());
    
    // ❌ Test 3: Invalid pattern (wrong parameter order)
    let invalid_order = original.check_pattern_compatibility(
        "data/{floor}/{building}/temp/{sensor_id}",
    );
    
    match invalid_order {
        Err(TopicPatternError::PatternStructureMismatch { original: orig, custom }) => {
            println!("✅ Correctly rejected pattern with wrong parameter order:");
            println!("   Original: {orig}");
            println!("   Custom:   {custom}");
        }
        _ => panic!("Expected PatternStructureMismatch error"),
    }
    
    // ❌ Test 4: Invalid pattern (wrong parameter names)
    let invalid_names = original.check_pattern_compatibility(
        "data/{building_id}/{floor}/temp/{sensor_id}",
    );
    
    match invalid_names {
        Err(TopicPatternError::PatternStructureMismatch { .. }) => {
            println!("✅ Correctly rejected pattern with wrong parameter names");
        }
        _ => panic!("Expected PatternStructureMismatch error"),
    }
    
    // ✅ Test 5: Compatible pattern with extra static segments (should work)
    let extra_static = original.check_pattern_compatibility(
        "data/{building}/{floor}/temp/celsius/{sensor_id}",
    );
    
    match extra_static {
        Ok(pattern) => {
            println!("✅ Correctly accepted pattern with extra static segments: {}", pattern.topic_pattern());
        }
        Err(_) => panic!("Should have accepted pattern with extra static segments"),
    }
    
    // ❌ Test 6: Invalid pattern (wildcard vs static mismatch)
    let invalid_type = original.check_pattern_compatibility(
        "sensors/building_a/{floor}/temp/{sensor_id}",
    );
    
    match invalid_type {
        Err(TopicPatternError::PatternStructureMismatch { .. }) => {
            println!("✅ Correctly rejected pattern with static vs wildcard mismatch");
        }
        _ => panic!("Expected PatternStructureMismatch error"),
    }
    
    println!("🎉 All pattern validation tests passed!");
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    test_pattern_validation()?;
    Ok(())
}
