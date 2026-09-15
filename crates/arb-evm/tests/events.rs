//! ABI fixtures are synthetic, not observed market events or deployment evidence.
use arb_evm::{PoolRegistry, events::*};
use serde_json::{Value, json};

fn pool() -> PoolRegistry {
    serde_json::from_str::<Vec<PoolRegistry>>(include_str!("fixtures/batch-registries.json"))
        .unwrap()
        .remove(0)
}
fn word(n: i128) -> String {
    let mut bytes = [if n < 0 { 255_u8 } else { 0 }; 32];
    bytes[16..].copy_from_slice(&n.to_be_bytes());
    format!("0x{}", hex::encode(bytes))
}
fn hash(n: u8) -> String {
    format!("0x{}", hex::encode([n; 32]))
}
fn log(signature: &str, indexed: &[i128], data: &[i128]) -> Value {
    let mut topics = vec![json!(signature)];
    topics.extend(indexed.iter().map(|n| json!(word(*n))));
    json!({"address":pool().pool,"blockNumber":"0x64","blockHash":hash(1),
           "transactionHash":hash(9),"transactionIndex":"0x0","logIndex":"0x5",
           "removed":false,"topics":topics,
           "data":format!("0x{}",data.iter().map(|n|word(*n)[2..].to_owned()).collect::<String>())})
}
fn swap() -> Value {
    log(SWAP, &[1, 2], &[9007199254740993, -7, 1_i128 << 96, 40, -1])
}
fn header(n: u64, id: u8, parent: u8) -> BlockHeader {
    BlockHeader {
        number: n,
        hash: hash(id),
        parent_hash: hash(parent),
        timestamp_seconds: n * 2,
    }
}

#[test]
fn swap_keeps_exact_signed_amounts_price_and_metadata() {
    let parsed = decode_log(&swap(), &pool()).unwrap();
    assert_eq!(parsed.block_number, 100);
    assert_eq!(parsed.log_index, 5);
    match parsed.decoded {
        PoolEvent::Swap {
            amount0,
            amount1,
            sqrt_price_x96,
            liquidity,
            tick,
            ..
        } => {
            assert_eq!(amount0, "9007199254740993");
            assert_eq!(amount1, "-7");
            assert_eq!(sqrt_price_x96, "79228162514264337593543950336");
            assert_eq!(liquidity, "40");
            assert_eq!(tick, -1);
        }
        _ => panic!("expected swap"),
    }
}

#[test]
fn int256_extremes_are_not_truncated_or_floated() {
    let mut value = swap();
    let data = value["data"].as_str().unwrap().to_owned();
    let min = format!("8{}", "0".repeat(63));
    let max = format!("7{}", "f".repeat(63));
    value["data"] = json!(format!("0x{min}{max}{}", &data[130..]));
    let parsed = decode_log(&value, &pool()).unwrap();
    if let PoolEvent::Swap {
        amount0, amount1, ..
    } = parsed.decoded
    {
        assert_eq!(
            amount0,
            "-57896044618658097711785492504343953926634992332820282019728792003956564819968"
        );
        assert_eq!(
            amount1,
            "57896044618658097711785492504343953926634992332820282019728792003956564819967"
        );
    } else {
        panic!("expected swap");
    }
}

#[test]
fn all_nine_abi_layouts_decode_without_guessing_missing_fields() {
    let cases = [
        (log(INITIALIZE, &[], &[1_i128 << 96, 0]), "Initialize"),
        (log(MINT, &[1, -10, 10], &[2, 9, 100, 200]), "Mint"),
        (log(COLLECT, &[1, -10, 10], &[2, 0, 0]), "Collect"),
        (log(BURN, &[1, -10, 10], &[0, 0, 0]), "Burn"),
        (swap(), "Swap"),
        (log(FLASH, &[1, 2], &[100, 200, 101, 201]), "Flash"),
        (
            log(ORACLE_CAPACITY, &[], &[16, 32]),
            "IncreaseObservationCardinalityNext",
        ),
        (log(PROTOCOL_FEE, &[], &[0, 0, 4, 10]), "SetFeeProtocol"),
        (
            log(COLLECT_PROTOCOL, &[1, 2], &[100, 200]),
            "CollectProtocol",
        ),
    ];
    for (value, name) in cases {
        assert_eq!(
            serde_json::to_value(decode_log(&value, &pool()).unwrap().decoded).unwrap()["event"],
            name
        );
    }
}

#[test]
fn removed_event_is_retained_for_explicit_rollback_handling() {
    let mut value = swap();
    value["removed"] = json!(true);
    assert!(decode_log(&value, &pool()).unwrap().removed);
    let wrapped = json!({"jsonrpc":"2.0","method":"eth_subscription","params":{"subscription":"sub1","result":value}});
    assert!(log_notification(&wrapped, "sub1", &pool()).unwrap().removed);
}

#[test]
fn unknown_or_incomplete_topics_never_turn_into_an_empty_event() {
    for replacement in [
        json!([]),
        json!([hash(4)]),
        json!([SWAP]),
        json!([SWAP, word(1)]),
        json!([SWAP, word(1), word(2), word(3)]),
        json!([SWAP, word(1), "0x01"]),
    ] {
        let mut value = swap();
        value["topics"] = replacement;
        assert!(decode_log(&value, &pool()).is_err());
    }
}

#[test]
fn incorrect_emitters_or_invalid_registries_are_rejected() {
    let mut value = swap();
    value["address"] = json!("0x0404040404040404040404040404040404040404");
    assert!(decode_log(&value, &pool()).is_err());
    let mut registry = pool();
    registry.tick_spacing = 0;
    assert!(decode_log(&swap(), &registry).is_err());
}

#[test]
fn missing_pending_and_noncanonical_metadata_fail_closed() {
    for key in [
        "blockNumber",
        "blockHash",
        "transactionHash",
        "transactionIndex",
        "logIndex",
        "removed",
    ] {
        let mut value = swap();
        value.as_object_mut().unwrap().remove(key);
        assert!(decode_log(&value, &pool()).is_err(), "missing {key}");
        value[key] = Value::Null;
        assert!(decode_log(&value, &pool()).is_err());
    }
    for quantity in ["0x+1", "0x01", "0x-1", "0x", "1", "0x10000000000000000"] {
        let mut value = swap();
        value["logIndex"] = json!(quantity);
        assert!(decode_log(&value, &pool()).is_err());
    }
    let mut value = swap();
    value["removed"] = json!(0);
    assert!(decode_log(&value, &pool()).is_err());
}

#[test]
fn exact_data_size_is_checked_before_hex_allocation() {
    for data in [
        "0x".into(),
        format!("0x{}", "0".repeat(322)),
        format!("0x{}", "0".repeat(1_000_000)),
        format!("0x{}", "z".repeat(320)),
    ] {
        let mut value = swap();
        value["data"] = json!(data);
        assert!(decode_log(&value, &pool()).is_err());
    }
}

#[test]
fn unsigned_padding_and_indexed_address_padding_are_checked() {
    let mut value = swap();
    let data = value["data"].as_str().unwrap().to_owned();
    for index in [2, 3] {
        // uint160 price, uint128 liquidity
        let mut changed = data.clone().into_bytes();
        changed[2 + index * 64] = b'1';
        value["data"] = json!(String::from_utf8(changed).unwrap());
        assert!(decode_log(&value, &pool()).is_err());
    }
    let mut value = swap();
    value["topics"][1] = json!(hash(1));
    assert!(decode_log(&value, &pool()).is_err());
}

#[test]
fn signed_tick_padding_bounds_and_position_spacing_are_checked() {
    let mut value = swap();
    let data = value["data"].as_str().unwrap().to_owned();
    let mut changed = data.into_bytes();
    changed[2 + 4 * 64] = b'0';
    value["data"] = json!(String::from_utf8(changed).unwrap());
    assert!(decode_log(&value, &pool()).is_err());
    assert!(decode_log(&log(INITIALIZE, &[], &[1_i128 << 96, 887273]), &pool()).is_err());
    for (lower, upper) in [(10, -10), (10, 10), (-9, 10), (-10, 9)] {
        assert!(decode_log(&log(BURN, &[1, lower, upper], &[0, 0, 0]), &pool()).is_err());
    }
}

#[test]
fn zero_collect_amounts_and_zero_owner_are_valid_abi_values() {
    let parsed = decode_log(&log(COLLECT, &[0, -10, 10], &[0, 0, 0]), &pool()).unwrap();
    if let PoolEvent::Collect {
        amount0,
        amount1,
        owner,
        ..
    } = parsed.decoded
    {
        assert_eq!(amount0, "0");
        assert_eq!(amount1, "0");
        assert_eq!(owner, format!("0x{}", "0".repeat(40)));
    } else {
        panic!("expected collect");
    }
}

#[test]
fn static_uint16_and_uint8_overflow_is_rejected() {
    assert!(decode_log(&log(ORACLE_CAPACITY, &[], &[65536, 1]), &pool()).is_err());
    assert!(decode_log(&log(PROTOCOL_FEE, &[], &[0, 256, 0, 0]), &pool()).is_err());
}

#[test]
fn new_head_requires_complete_identity_and_valid_subscription() {
    let h = json!({"number":"0x64","hash":hash(1),"parentHash":hash(2),"timestamp":"0xc8"});
    let v = json!({"jsonrpc":"2.0","method":"eth_subscription","params":{"subscription":"heads","result":h}});
    assert_eq!(head_notification(&v, "heads").unwrap().number, 100);
    for expected in ["other", ""] {
        assert!(head_notification(&v, expected).is_err());
    }
    for (key, val) in [
        ("id", json!(1)),
        ("error", Value::Null),
        ("jsonrpc", json!("1.0")),
        ("method", json!("eth_getLogs")),
    ] {
        let mut bad = v.clone();
        bad[key] = val;
        assert!(head_notification(&bad, "heads").is_err());
    }
    for key in ["number", "hash", "parentHash", "timestamp"] {
        let mut bad = v.clone();
        bad["params"]["result"].as_object_mut().unwrap().remove(key);
        assert!(head_notification(&bad, "heads").is_err());
    }
}

#[test]
fn head_relations_distinguish_duplicate_gap_and_changed_ancestry() {
    let previous = header(100, 10, 9);
    assert_eq!(
        head_change(&previous, &previous).unwrap(),
        HeadChange::Duplicate
    );
    assert_eq!(
        head_change(&previous, &header(101, 11, 10)).unwrap(),
        HeadChange::Extension
    );
    assert_eq!(
        head_change(&previous, &header(104, 14, 13)).unwrap(),
        HeadChange::Gap {
            first_missing: 101,
            last_missing: 103
        }
    );
    for next in [header(101, 11, 44), header(100, 22, 9), header(99, 9, 8)] {
        assert_eq!(
            head_change(&previous, &next).unwrap(),
            HeadChange::ReorgOrOutOfOrder
        );
    }
    let mut next = header(101, 11, 10);
    next.timestamp_seconds = 1;
    assert_eq!(
        head_change(&previous, &next).unwrap(),
        HeadChange::ReorgOrOutOfOrder
    );
    let max = BlockHeader {
        number: u64::MAX,
        ..previous.clone()
    };
    assert_eq!(head_change(&max, &max).unwrap(), HeadChange::Duplicate);
}

#[test]
fn same_hash_with_different_header_fields_is_not_a_new_extension() {
    let prior = header(100, 10, 9);
    for next in [header(101, 10, 10), header(104, 10, 13)] {
        assert_eq!(
            head_change(&prior, &next).unwrap(),
            HeadChange::ReorgOrOutOfOrder
        );
    }
}
