//! Bounded deterministic same-chain research evaluation.
//! Inputs must come from verified capture bundles admitted to the same frozen session.
//! Registry syntax and successful math do not grant quote or execution qualification.
use arb_adapter_api::StateContext;
use arb_config::ValidatedConfig;
use arb_domain::{
    AssetId, AtomicAmount, DatasetOrigin, DecisionCaptureRef, DecisionGrouping, DecisionLeg,
    DecisionResult, DecisionTrace, NetworkId, PoolId, SignedAmount, DECISION_SCHEMA_VERSION,
};
use std::{collections::HashSet, fmt};

pub const CALCULATION_VERSION: &str = "capture-pair-research-v1;bounds8x63;group1000";
pub const MAX_POOLS: usize = 8;
pub const MAX_TRACES: usize = 64;
pub const MAX_BATCH_BYTES: usize = 256 * 1024;
pub const GROUPING_WINDOW_MS: u64 = 1000;
const SUMMARY_BYTE_RESERVE: usize = 16 * 1024;

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // At most eight owned verified pool snapshots per evaluation.
pub enum PoolState {
    Base { snapshot: arb_evm::PoolSnapshot, registry: arb_evm::PoolRegistry },
    Solana { snapshot: arb_solana::PoolSnapshot, registry: arb_solana::PoolRegistry },
}
#[derive(Clone, Debug)]
pub struct CapturedPool {
    pub capture: DecisionCaptureRef,
    pub origin: DatasetOrigin,
    pub configuration_digest: String,
    pub state: PoolState,
}
pub struct EvaluationRequest<'a> {
    pub session_id: &'a str,
    pub experiment_id: &'a str,
    pub generation: u64,
    pub configuration: &'a ValidatedConfig,
    pub strategy_id: &'a str,
    pub network_id: NetworkId,
    pub dataset_origin: DatasetOrigin,
    pub observed_at_unix_ms: u64,
    /// Original batch monotonic observation, before capture or queue waiting.
    pub observed_monotonic_ms: u64,
    pub deadline_monotonic_ms: u64,
    pub pools: &'a [CapturedPool],
}
#[derive(Clone, Copy, Debug)]
pub struct GateState {
    pub generation: u64,
    pub admission_open: bool,
    pub now_monotonic_ms: u64,
}
pub trait EvaluationGate { fn state(&self) -> GateState; }
impl EvaluationGate for GateState { fn state(&self) -> GateState { *self } }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineError { pub code: &'static str }
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.code) }
}
impl std::error::Error for EngineError {}
fn error(code: &'static str) -> EngineError { EngineError { code } }

/// Every existing protocol remains unqualified for full atomic simulation and
/// deployed-program equivalence. A configured registry cannot promote these bits.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct CapabilityReport {
    pub network_id: NetworkId,
    pub venue_family: &'static str,
    pub read_decode_implemented: bool,
    pub research_math_implemented: bool,
    pub qualified_quote: bool,
    pub transaction_build: bool,
    pub full_transaction_simulation: bool,
    pub submit: bool,
    pub reason_codes: Vec<&'static str>,
}
pub fn capabilities(network_id: NetworkId) -> CapabilityReport {
    CapabilityReport {
        network_id, venue_family:venue(network_id),
        read_decode_implemented:true,research_math_implemented:true,qualified_quote:false,
        transaction_build:false,full_transaction_simulation:false,submit:false,
        reason_codes:vec!["CURRENT_PROTOCOL_EQUIVALENCE_UNQUALIFIED","TOKEN_BEHAVIOR_UNQUALIFIED","FULL_TRANSACTION_SIMULATION_NOT_RUN"],
    }
}
fn venue(network: NetworkId) -> &'static str {
    match network {NetworkId::BaseMainnet=>"uniswap-v3",NetworkId::SolanaMainnet=>"orca-whirlpools"}
}
impl PoolState {
    pub fn network_id(&self) -> NetworkId {
        match self {Self::Base {..}=>NetworkId::BaseMainnet,Self::Solana {..}=>NetworkId::SolanaMainnet}
    }
    fn identities(&self) -> Result<(PoolId,AssetId,AssetId),EngineError> {
        let network=self.network_id();
        let (pool,a,b)=match self {
            Self::Base {snapshot,registry}=>{
                registry.validate().map_err(|_|error("CAPTURE_IDENTITY_MISMATCH"))?;
                if !snapshot.pool.eq_ignore_ascii_case(&registry.pool) {return Err(error("CAPTURE_IDENTITY_MISMATCH"));}
                (snapshot.pool.as_str(),registry.token0.as_str(),registry.token1.as_str())
            },
            Self::Solana {snapshot,registry}=>{
                registry.validate().map_err(|_|error("CAPTURE_IDENTITY_MISMATCH"))?;
                if snapshot.pool!=registry.pool || snapshot.state.mint_a!=registry.mint_a
                    || snapshot.state.mint_b!=registry.mint_b || snapshot.state.vault_a!=registry.vault_a
                    || snapshot.state.vault_b!=registry.vault_b || snapshot.state.whirlpools_config!=registry.whirlpools_config {
                    return Err(error("CAPTURE_IDENTITY_MISMATCH"));
                }
                (snapshot.pool.as_str(),registry.mint_a.as_str(),registry.mint_b.as_str())
            }
        };
        Ok((PoolId::new(network,pool).map_err(|_|error("CAPTURE_IDENTITY_MISMATCH"))?,
            AssetId::new(network,a).map_err(|_|error("CAPTURE_IDENTITY_MISMATCH"))?,
            AssetId::new(network,b).map_err(|_|error("CAPTURE_IDENTITY_MISMATCH"))?))
    }
    fn context(&self) -> &StateContext {
        match self {Self::Base {snapshot,..}=>&snapshot.context,Self::Solana {snapshot,..}=>&snapshot.context}
    }
}
struct PoolView<'a> { input:&'a CapturedPool, id:PoolId, a:AssetId, b:AssetId }
struct RouteView<'a> { first:&'a PoolView<'a>, second:&'a PoolView<'a>, start:AssetId, middle:AssetId }
struct LegQuote { output:AtomicAmount, fee:AtomicAmount }
trait Quoter {
    fn quote(&self,pool:&PoolState,input:&AtomicAmount,asset:&AssetId)->Result<LegQuote,EngineError>;
}
struct ProtocolMath;
impl Quoter for ProtocolMath {
    fn quote(&self,pool:&PoolState,input:&AtomicAmount,asset:&AssetId)->Result<LegQuote,EngineError> {
        match pool {
            PoolState::Base {snapshot,registry}=>{
                let q=arb_evm::math::quote_exact_input_math(snapshot,registry,input.clone(),asset.address().eq_ignore_ascii_case(&registry.token0))
                    .map_err(|failure| map_math_error(failure.0))?;
                if !q.input_asset.eq_ignore_ascii_case(asset.address()) || q.amount_in!=*input {
                    return Err(error("ASSET_CONTINUITY_MISMATCH"));
                }
                Ok(LegQuote {output:q.amount_out,fee:q.pool_fee_in_input_asset})
            }
            PoolState::Solana {snapshot,..}=>{
                let value=input.to_string().parse::<u64>().map_err(|_|error("UNSUPPORTED_SIZE"))?;
                let q=arb_solana::math::quote_exact_input_math(snapshot,value,asset.address()==snapshot.state.mint_a)
                    .map_err(|failure| map_math_error(failure.0))?;
                if q.input_asset!=asset.address() || q.amount_in!=input.to_string() {
                    return Err(error("ASSET_CONTINUITY_MISMATCH"));
                }
                Ok(LegQuote {
                    output:q.amount_out.parse().map_err(|_|error("ARITHMETIC_OVERFLOW"))?,
                    fee:q.pool_fee_in_input_asset.parse().map_err(|_|error("ARITHMETIC_OVERFLOW"))?,
                })
            }
        }
    }
}
fn map_math_error(message:&'static str)->EngineError {
    // This maps trusted static adapter errors to the public code catalog. No
    // external message, endpoint or RPC body is copied into decision diagnostics.
    let code=if message.contains("coverage") || message.contains("window") || message.contains("tick array") || message.contains("bitmap") {
        "INCOMPLETE_TICK_COVERAGE"
    } else if message.contains("overflow") {
        "ARITHMETIC_OVERFLOW"
    } else if message.contains("zero output") {
        "OUTPUT_ROUNDS_TO_ZERO"
    } else {"MATH_INPUT_REJECTED"};
    error(code)
}
fn check_gate(request:&EvaluationRequest<'_>,gate:&impl EvaluationGate)->Result<u64,EngineError> {
    let state=gate.state();
    if !state.admission_open || state.generation!=request.generation {return Err(error("WORK_GENERATION_CANCELLED"));}
    if state.now_monotonic_ms>=request.deadline_monotonic_ms {return Err(error("DEADLINE_EXPIRED"));}
    state.now_monotonic_ms.checked_sub(request.observed_monotonic_ms).ok_or_else(||error("STALE_INPUT"))
}
fn same_context(first:&StateContext,second:&StateContext)->bool {
    match (first,second) {
        (StateContext::Evm {block_number:a,block_hash:b,parent_hash:c,block_timestamp_seconds:d,finality:e},
         StateContext::Evm {block_number:f,block_hash:g,parent_hash:h,block_timestamp_seconds:i,finality:j})=>a==f&&b==g&&c==h&&d==i&&e==j,
        (StateContext::Solana {slot:a,genesis_hash:b,commitment:c,account_context:d},
         StateContext::Solana {slot:e,genesis_hash:f,commitment:g,account_context:h})=>a==e&&b==f&&c==g&&d==h,
        _=>false,
    }
}
fn base_trace(request:&EvaluationRequest<'_>,captures:Vec<DecisionCaptureRef>,result:DecisionResult)->DecisionTrace {
    DecisionTrace {
        schema_version:DECISION_SCHEMA_VERSION.into(),observation_id:String::new(),
        session_id:request.session_id.into(),experiment_id:request.experiment_id.into(),generation:request.generation,
        configuration_digest:request.configuration.digest().into(),calculation_version:CALCULATION_VERSION.into(),
        strategy_id:request.strategy_id.into(),network_id:request.network_id,mode:request.configuration.mode(),
        source_kind:request.dataset_origin.source_kind(),dataset_origin:request.dataset_origin,
        observed_at_unix_ms:request.observed_at_unix_ms,input_age_ms:None,capture_refs:captures,route:Vec::new(),
        amount_in_minor:None,result,
        grouping:DecisionGrouping {version:String::new(),key:String::new(),window_ms:GROUPING_WINDOW_MS,window_start_ms:0},
        diagnostics:Vec::new(),
    }
}
fn seal(trace:DecisionTrace)->Result<DecisionTrace,EngineError> {
    trace.seal().map_err(|_|error("CAPTURE_IDENTITY_MISMATCH"))
}
fn coverage(request:&EvaluationRequest<'_>,refs:Vec<DecisionCaptureRef>,code:&'static str,no_route:bool)->Result<DecisionTrace,EngineError> {
    let result=if no_route&&!refs.is_empty() {DecisionResult::NoRoute {reason_codes:vec![code.into()]}}
        else {DecisionResult::DataUnavailable {reason_codes:vec![code.into()]}};
    seal(base_trace(request,refs,result))
}
fn route_trace(request:&EvaluationRequest<'_>,route:&RouteView<'_>,amount:&AtomicAmount,age:u64,result:DecisionResult)->Result<DecisionTrace,EngineError> {
    let mut trace=base_trace(request,vec![route.first.input.capture.clone(),route.second.input.capture.clone()],result);
    trace.input_age_ms=Some(age);trace.amount_in_minor=Some(amount.clone());
    trace.route=vec![
        DecisionLeg {pool_id:route.first.id.clone(),asset_in:route.start.clone(),asset_out:route.middle.clone(),venue_family:venue(request.network_id).into()},
        DecisionLeg {pool_id:route.second.id.clone(),asset_in:route.middle.clone(),asset_out:route.start.clone(),venue_family:venue(request.network_id).into()},
    ];
    trace.diagnostics=vec!["RESEARCH_MATH_ONLY".into(),"CURRENT_PROTOCOL_EQUIVALENCE_UNQUALIFIED".into(),
        "TOKEN_BEHAVIOR_UNQUALIFIED".into(),"EXTERNAL_COSTS_UNAVAILABLE".into(),"FULL_TRANSACTION_SIMULATION_NOT_RUN".into(),
        "SNAPSHOT_NOT_ATOMIC".into()];
    seal(trace)
}
/// Pure bounded evaluation, without I/O or funds. The caller must repeat the
/// generation/lease fence in the same database transaction that appends the batch.
/// A cancellation/deadline error publishes no partial batch.
pub fn evaluate(request:&EvaluationRequest<'_>,gate:&impl EvaluationGate)->Result<Vec<DecisionTrace>,EngineError> {
    evaluate_with(request,gate,&ProtocolMath)
}
fn evaluate_with(request:&EvaluationRequest<'_>,gate:&impl EvaluationGate,quoter:&impl Quoter)->Result<Vec<DecisionTrace>,EngineError> {
    let output=evaluate_inner(request,gate,quoter)?;
    check_gate(request,gate)?;
    if serde_json::to_vec(&output).map_err(|_|error("EVALUATION_BUDGET_EXHAUSTED"))?.len()>MAX_BATCH_BYTES {
        return Err(error("EVALUATION_BUDGET_EXHAUSTED"));
    }
    Ok(output)
}
fn evaluate_inner(request:&EvaluationRequest<'_>,gate:&impl EvaluationGate,quoter:&impl Quoter)->Result<Vec<DecisionTrace>,EngineError> {
    check_gate(request,gate)?;
    if !request.configuration.network_enabled(request.network_id)
        || !request.configuration.strategy_ids().iter().any(|s|s==request.strategy_id) {
        return Err(error("UNSUPPORTED_REQUESTED_CAPABILITY"));
    }
    if request.pools.len()>MAX_POOLS {
        return Ok(vec![coverage(request,Vec::new(),"MAX_POOL_BOUND_EXCEEDED",false)?]);
    }
    let mut views=Vec::new();
    let mut seen=HashSet::new();
    let mut refs=Vec::new();
    for pool in request.pools {
        check_gate(request,gate)?;
        if pool.configuration_digest!=request.configuration.digest() {return Err(error("CAPTURE_CONFIGURATION_MISMATCH"));}
        if pool.origin!=request.dataset_origin {return Err(error("CAPTURE_ORIGIN_MISMATCH"));}
        if pool.state.network_id()!=request.network_id {return Err(error("CAPTURE_IDENTITY_MISMATCH"));}
        match &pool.state {
            PoolState::Base {snapshot,..} if !matches!(snapshot.context,StateContext::Evm {..})=>{
                return Err(error("CAPTURE_CONTEXT_MISMATCH"));
            },
            PoolState::Solana {snapshot,registry}=>{
                if registry.expected_genesis_hash!=request.configuration.expected_genesis_identity()
                    || !matches!(&snapshot.context,StateContext::Solana {genesis_hash,..} if genesis_hash==&registry.expected_genesis_hash)
                    || snapshot.tick_arrays.iter().any(|array|!registry.tick_arrays.contains(&array.address)) {
                    return Err(error("CAPTURE_CONTEXT_MISMATCH"));
                }
            },
            _=>{},
        }
        let (id,a,b)=pool.state.identities()?;
        if !request.configuration.verified_pools(request.network_id).contains(&id)
            || !request.configuration.verified_assets(request.network_id).contains(&a)
            || !request.configuration.verified_assets(request.network_id).contains(&b) {
            return Err(error("POOLS_OUTSIDE_CONFIG"));
        }
        if !seen.insert(id.to_string()) {return Err(error("DISTINCT_POOL_REQUIRED"));}
        refs.push(pool.capture.clone());
        views.push(PoolView {input:pool,id,a,b});
    }
    views.sort_by_key(|p|p.id.to_string());
    refs.sort_by(|a,b|a.capture_id.cmp(&b.capture_id));
    if views.is_empty() {return Ok(vec![coverage(request,refs,"NO_CAPTURE_INPUTS",false)?]);}
    let Some(start)=request.configuration.starting_asset(request.network_id) else {
        return Ok(vec![coverage(request,refs,"NO_CONFIGURED_START_ASSET",true)?]);
    };
    let sizes=request.configuration.trade_sizes();
    if sizes.is_empty() {return Ok(vec![coverage(request,refs,"NO_CONFIGURED_TRADE_SIZES",true)?]);}
    let mut routes=Vec::new();
    for first in &views {
        let middle=if &first.a==start {&first.b} else if &first.b==start {&first.a} else {continue};
        for second in &views {
            if first.id!=second.id && ((&second.a==middle&&&second.b==start)||(&second.b==middle&&&second.a==start)) {
                routes.push(RouteView {first,second,start:start.clone(),middle:middle.clone()});
            }
        }
    }
    if routes.is_empty() {return Ok(vec![coverage(request,refs,"NO_ELIGIBLE_POOL_PAIRS",true)?]);}
    let limit=(request.configuration.maximum_queued_evaluations() as usize).min(MAX_TRACES-1);
    let mut output=Vec::new();
    let mut bytes=2_usize;
    let mut exhausted=false;
    'routes: for route in &routes {
        for amount in sizes {
            let age=check_gate(request,gate)?;
            if output.len()>=limit {exhausted=true;break 'routes;}
            let quoted=if age>u64::from(request.configuration.maximum_state_age_ms()) {
                Err(error("STALE_INPUT"))
            } else if !same_context(route.first.input.state.context(),route.second.input.state.context()) {
                Err(error("CAPTURE_CONTEXT_MISMATCH"))
            } else {
                let first=quoter.quote(&route.first.input.state,amount,&route.start);
                check_gate(request,gate)?;
                match first {
                    Ok(first) if !first.output.is_zero()=>{
                        let second=quoter.quote(&route.second.input.state,&first.output,&route.middle);
                        check_gate(request,gate)?;
                        second.map(|second|(second.output,first.fee,second.fee))
                    },
                    Ok(_)=>Err(error("OUTPUT_ROUNDS_TO_ZERO")),
                    Err(failure)=>Err(failure),
                }
            };
            let final_age=check_gate(request,gate)?;
            let quoted=if final_age>u64::from(request.configuration.maximum_state_age_ms()) {Err(error("STALE_INPUT"))} else {quoted};
            let result=match quoted {
                Ok((out,first_fee,second_fee)) if !out.is_zero()=>DecisionResult::Quoted {
                    gross_delta_minor:SignedAmount::difference(&out,amount),quoted_output_minor:out,
                    included_pool_fees:vec![first_fee,second_fee],
                },
                Ok(_)=>DecisionResult::Rejected {reason_codes:vec!["OUTPUT_ROUNDS_TO_ZERO".into()]},
                Err(failure)=>DecisionResult::Rejected {reason_codes:vec![failure.code.into()]},
            };
            let trace=route_trace(request,route,amount,final_age,result)?;
            let size=serde_json::to_vec(&trace).map_err(|_|error("EVALUATION_BUDGET_EXHAUSTED"))?.len()+1;
            if bytes+size>MAX_BATCH_BYTES-SUMMARY_BYTE_RESERVE {exhausted=true;break 'routes;}
            bytes+=size;output.push(trace);
        }
    }
    if exhausted {output.push(coverage(request,refs,"EVALUATION_BUDGET_EXHAUSTED",false)?);}
    check_gate(request,gate)?;
    if serde_json::to_vec(&output).map_err(|_|error("EVALUATION_BUDGET_EXHAUSTED"))?.len()>MAX_BATCH_BYTES {
        return Err(error("EVALUATION_BUDGET_EXHAUSTED"));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arb_adapter_api::SnapshotQuality;
    use std::{cell::Cell,collections::BTreeMap};
    fn address(n:u64)->String {format!("0x{n:040x}")}
    fn base_pool(n:u64)->CapturedPool {
        let pool=address(n);
        let registry=arb_evm::PoolRegistry {
            schema_version:1,pool:pool.clone(),token0:address(1),token1:address(2),fee:500,tick_spacing:10,
            pool_runtime_sha256:format!("sha256:{}","0".repeat(64)),
            factory_runtime_sha256:format!("sha256:{}","0".repeat(64)),
            qualification_reference:"synthetic math fixture".into(),bitmap_word_min:-1,bitmap_word_max:1,
        };
        let snapshot=arb_evm::PoolSnapshot {
            pool,context:StateContext::Evm {block_number:1,block_hash:format!("0x{}","a".repeat(64)),
                parent_hash:format!("0x{}","b".repeat(64)),block_timestamp_seconds:1_700_000_000,finality:"finalized".into()},
            sqrt_price_x96_hex:"0x0000000000000001000000000000000000000000".into(),
            tick:0,liquidity:"1000000000000".into(),
            bitmap_words:(-1..=1).map(|n|(n,format!("0x{}","0".repeat(64)))).collect::<BTreeMap<_,_>>(),
            initialized_ticks:BTreeMap::new(),
            quality:SnapshotQuality {coherent:false,complete_for_quote:false,quote_implementation_qualified:false,
                observed_at_ms:1_700_000_000_000,max_age_ms:1000,reasons:vec!["synthetic-fixture".into()]},
        };
        let digest=format!("sha256:{n:064x}");
        CapturedPool {
            capture:DecisionCaptureRef {capture_id:format!("synthetic-{n}"),manifest_digest:digest.clone(),snapshot_id:digest},
            origin:DatasetOrigin::ManuallyConstructed,configuration_digest:String::new(),
            state:PoolState::Base {snapshot,registry},
        }
    }
    fn config(pools:&mut [CapturedPool],sizes:&[u64])->ValidatedConfig {
        let inert=ValidatedConfig::from_toml(include_str!("../../../config/research.example.toml")).unwrap();
        let mut json:serde_json::Value=serde_json::from_str(inert.effective_json()).unwrap();
        json["research"]["trade_sizes_minor"]=serde_json::json!(sizes.iter().map(ToString::to_string).collect::<Vec<_>>());
        json["networks"]["base"]["enabled"]=serde_json::json!(true);
        json["networks"]["base"]["verified_pool_ids"]=serde_json::json!(
            pools.iter().map(|p|p.state.identities().unwrap().0.to_string()).collect::<Vec<_>>());
        let mut assets=pools.iter().flat_map(|p|{let (_,a,b)=p.state.identities().unwrap();[a.to_string(),b.to_string()]}).collect::<Vec<_>>();
        assets.sort();assets.dedup();
        json["networks"]["base"]["verified_asset_ids"]=serde_json::json!(assets);
        json["networks"]["base"]["starting_asset_id"]=serde_json::json!(format!("base-mainnet:{}",address(1)));
        json["networks"]["base"]["rpc_secret_reference"]=serde_json::json!("env:BASE_RPC_URL");
        json["networks"]["base"]["registry_qualification_digest"]=serde_json::json!(format!("sha256:{}","a".repeat(64)));
        let config=ValidatedConfig::from_effective_json(&json.to_string()).unwrap();
        for pool in pools {pool.configuration_digest=config.digest().into();}
        config
    }
    fn request<'a>(config:&'a ValidatedConfig,pools:&'a [CapturedPool])->EvaluationRequest<'a> {
        EvaluationRequest {
            session_id:"synthetic-session",experiment_id:"manual-pair",generation:7,configuration:config,
            strategy_id:"cyclic-exact-in-2leg-v1",network_id:NetworkId::BaseMainnet,
            dataset_origin:DatasetOrigin::ManuallyConstructed,observed_at_unix_ms:1_700_000_000_000,
            observed_monotonic_ms:0,deadline_monotonic_ms:10000,pools,
        }
    }
    fn gate()->GateState {GateState {generation:7,admission_open:true,now_monotonic_ms:10}}
    #[test]
    fn actual_base_math_produces_two_loss_candidates_with_unknown_external_costs() {
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[1_000_000]);
        let traces=evaluate(&request(&config,&pools),&gate()).unwrap();
        assert_eq!(traces.len(),2);
        for trace in traces {
            let DecisionResult::Quoted {gross_delta_minor,..}=&trace.result else {panic!("actual protocol math must quote");};
            assert!(gross_delta_minor.to_string().starts_with('-'));
            let opportunity=trace.to_opportunity().unwrap().unwrap();
            assert_eq!(opportunity.evidence_label,arb_domain::Evidence::Candidate);
            assert!(opportunity.net_after_explicit_costs_minor.is_none());
            assert!(!opportunity.snapshot.consistent);
            assert_eq!(opportunity.dataset_origin,Some(DatasetOrigin::ManuallyConstructed));
        }
    }
    #[test]
    fn deterministic_pool_and_size_order_preserves_byte_identical_observations() {
        let mut a=vec![base_pool(3),base_pool(4)];
        let config_a=config(&mut a,&[1_000_000,2_000_000]);
        let mut b=vec![base_pool(4),base_pool(3)];
        let config_b=config(&mut b,&[2_000_000,1_000_000]);
        assert_eq!(config_a.digest(),config_b.digest());
        let first=evaluate(&request(&config_a,&a),&gate()).unwrap();
        let next=evaluate(&request(&config_b,&b),&gate()).unwrap();
        assert_eq!(first,next);
        assert_eq!(first.len(),4);
        assert_ne!(first[0].grouping.key,first[1].grouping.key);
    }
    #[test]
    fn mismatched_context_and_stale_data_are_rejections_without_fake_outputs() {
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[100_000]);
        if let PoolState::Base {snapshot,..}=&mut pools[1].state {
            if let StateContext::Evm {block_number,..}=&mut snapshot.context {*block_number=2;}
        }
        let result=evaluate(&request(&config,&pools),&gate()).unwrap();
        assert!(result.iter().all(|t|matches!(&t.result,DecisionResult::Rejected {reason_codes} if reason_codes==&["CAPTURE_CONTEXT_MISMATCH"])));
        let mut stale=gate();stale.now_monotonic_ms=1001;
        let result=evaluate(&request(&config,&pools),&stale).unwrap();
        assert!(result.iter().all(|t|matches!(&t.result,DecisionResult::Rejected {reason_codes} if reason_codes==&["STALE_INPUT"])));
        assert!(result.iter().all(|t|t.to_opportunity().unwrap().is_none()));
    }
    #[test]
    fn missing_tick_coverage_is_missing_evidence_not_zero_return() {
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[1_000_000]);
        for pool in &mut pools {
            if let PoolState::Base {snapshot,..}=&mut pool.state {snapshot.bitmap_words.clear();}
        }
        let result=evaluate(&request(&config,&pools),&gate()).unwrap();
        assert_eq!(result.len(),2);
        assert!(result.iter().all(|t|matches!(&t.result,DecisionResult::Rejected {..})));
        assert!(result.iter().all(|t|t.to_opportunity().unwrap().is_none()));
    }
    #[test]
    fn capacity_is_explicit_and_serialized_batch_is_bounded() {
        let mut pools=(3..=10).map(base_pool).collect::<Vec<_>>();
        let config=config(&mut pools,&[100_000,200_000,300_000,400_000]);
        let result=evaluate(&request(&config,&pools),&gate()).unwrap();
        assert!(result.len()<=MAX_TRACES);
        assert!(matches!(&result.last().unwrap().result,DecisionResult::DataUnavailable {reason_codes} if reason_codes==&["EVALUATION_BUDGET_EXHAUSTED"]));
        assert!(serde_json::to_vec(&result).unwrap().len()<=MAX_BATCH_BYTES);
        assert!(result.iter().take(result.len()-1).all(|t|t.route.len()==2));
    }
    #[test]
    fn missing_inputs_and_absent_cycles_have_separate_denominators() {
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[100_000]);
        let empty=evaluate(&request(&config,&[]),&gate()).unwrap();
        assert!(matches!(empty[0].result,DecisionResult::DataUnavailable {..}));
        assert!(empty[0].capture_refs.is_empty());
        let one=evaluate(&request(&config,&pools[..1]),&gate()).unwrap();
        assert!(matches!(one[0].result,DecisionResult::NoRoute {..}));
        assert_eq!(one[0].capture_refs.len(),1);
        assert!(one[0].amount_in_minor.is_none());
    }
    #[test]
    fn duplicate_pool_config_mismatch_and_origin_spoof_are_not_admitted() {
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[100_000]);
        let mut duplicate=pools.clone();duplicate[1]=duplicate[0].clone();
        assert_eq!(evaluate(&request(&config,&duplicate),&gate()).unwrap_err().code,"DISTINCT_POOL_REQUIRED");
        let mut bad=pools.clone();bad[0].configuration_digest=format!("sha256:{}","b".repeat(64));
        assert_eq!(evaluate(&request(&config,&bad),&gate()).unwrap_err().code,"CAPTURE_CONFIGURATION_MISMATCH");
        let mut bad=pools;bad[0].origin=DatasetOrigin::RecordedLive;
        assert_eq!(evaluate(&request(&config,&bad),&gate()).unwrap_err().code,"CAPTURE_ORIGIN_MISMATCH");
    }
    struct ChangingGate {state:Cell<GateState>}
    impl EvaluationGate for ChangingGate {fn state(&self)->GateState {self.state.get()}}
    struct CancellingQuoter<'a> {gate:&'a ChangingGate,deadline:bool}
    impl Quoter for CancellingQuoter<'_> {
        fn quote(&self,_pool:&PoolState,input:&AtomicAmount,_asset:&AssetId)->Result<LegQuote,EngineError> {
            let mut state=self.gate.state.get();
            if self.deadline {state.now_monotonic_ms=10000;} else {state.generation+=1;}
            self.gate.state.set(state);
            Ok(LegQuote {output:input.clone(),fee:AtomicAmount::from(0)})
        }
    }
    #[test]
    fn generation_change_and_deadline_between_legs_discard_the_entire_batch() {
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[100_000]);
        for (deadline,code) in [(false,"WORK_GENERATION_CANCELLED"),(true,"DEADLINE_EXPIRED")] {
            let gate=ChangingGate {state:Cell::new(gate())};
            let quoter=CancellingQuoter {gate:&gate,deadline};
            assert_eq!(evaluate_with(&request(&config,&pools),&gate,&quoter).unwrap_err().code,code);
        }
    }
    #[test]
    fn static_capabilities_cannot_be_promoted_by_configured_registry_or_quality_flags() {
        for network in [NetworkId::BaseMainnet,NetworkId::SolanaMainnet] {
            let capability=capabilities(network);
            assert!(capability.read_decode_implemented&&capability.research_math_implemented);
            assert!(!capability.qualified_quote&&!capability.full_transaction_simulation&&!capability.transaction_build&&!capability.submit);
        }
        let mut pools=vec![base_pool(3),base_pool(4)];
        let config=config(&mut pools,&[100_000]);
        for pool in &mut pools {
            if let PoolState::Base {snapshot,..}=&mut pool.state {
                snapshot.quality.quote_implementation_qualified=true;
                snapshot.quality.coherent=true;snapshot.quality.complete_for_quote=true;
            }
        }
        for trace in evaluate(&request(&config,&pools),&gate()).unwrap() {
            assert_eq!(trace.to_opportunity().unwrap().unwrap().evidence_label,arb_domain::Evidence::Candidate);
        }
    }
    fn solana_pool(n:u8)->CapturedPool {
        let key=|n:u8|bs58::encode([n;32]).into_string();
        let registry=arb_solana::PoolRegistry {
            schema_version:1,expected_genesis_hash:key(11),pool:key(n),whirlpools_config:key(10),
            program_data:key(9),program_data_sha256:format!("sha256:{}","0".repeat(64)),
            mint_a:key(1),mint_b:key(2),decimals_a:6,decimals_b:6,
            vault_a:key(n+20),vault_b:key(n+30),
            tick_arrays:(0..3).map(|i|key(n*10+40+i)).collect(),
            qualification_reference:"manually constructed historical Orca math fixture".into(),
        };
        let snapshot=arb_solana::PoolSnapshot {
            pool:registry.pool.clone(),
            context:StateContext::Solana {slot:42,genesis_hash:registry.expected_genesis_hash.clone(),
                commitment:"finalized".into(),account_context:"synthetic-single-response".into()},
            state:arb_solana::WhirlpoolState {
                whirlpools_config:registry.whirlpools_config.clone(),tick_spacing:64,fee_tier_index:64,
                fee_rate:500,protocol_fee_rate:0,liquidity:"1000000000000".into(),
                sqrt_price_x64:"18446744073709551616".into(),tick_current_index:0,
                mint_a:registry.mint_a.clone(),mint_b:registry.mint_b.clone(),
                vault_a:registry.vault_a.clone(),vault_b:registry.vault_b.clone(),
            },
            tick_arrays:registry.tick_arrays.iter().zip([-5632,0,5632]).map(|(address,start_tick_index)|
                arb_solana::FixedTickArray {address:address.clone(),start_tick_index,initialized_ticks:Vec::new()}).collect(),
            account_write_provenance:"synthetic fixture".into(),
            quality:SnapshotQuality {coherent:false,complete_for_quote:false,quote_implementation_qualified:false,
                observed_at_ms:1_700_000_000_000,max_age_ms:1000,reasons:vec!["manual-fixture".into()]},
        };
        let digest=format!("sha256:{n:064x}");
        CapturedPool {
            capture:DecisionCaptureRef {capture_id:format!("manual-solana-{n}"),manifest_digest:digest.clone(),snapshot_id:digest},
            origin:DatasetOrigin::ManuallyConstructed,configuration_digest:String::new(),
            state:PoolState::Solana {snapshot,registry},
        }
    }
    #[test]
    fn actual_solana_math_routes_are_candidate_only_and_large_input_is_rejected() {
        let mut pools=vec![solana_pool(3),solana_pool(4)];
        let inert=ValidatedConfig::from_toml(include_str!("../../../config/research.example.toml")).unwrap();
        let mut json:serde_json::Value=serde_json::from_str(inert.effective_json()).unwrap();
        json["research"]["trade_sizes_minor"]=serde_json::json!(["1000000","18446744073709551616"]);
        json["networks"]["solana"]["enabled"]=serde_json::json!(true);
        json["networks"]["solana"]["verified_pool_ids"]=serde_json::json!(pools.iter().map(|p|p.state.identities().unwrap().0.to_string()).collect::<Vec<_>>());
        let (_,a,b)=pools[0].state.identities().unwrap();
        json["networks"]["solana"]["verified_asset_ids"]=serde_json::json!([a.to_string(),b.to_string()]);
        json["networks"]["solana"]["starting_asset_id"]=serde_json::json!(a.to_string());
        json["networks"]["solana"]["rpc_secret_reference"]=serde_json::json!("env:SOLANA_RPC_URL");
        json["networks"]["solana"]["registry_qualification_digest"]=serde_json::json!(format!("sha256:{}","a".repeat(64)));
        json["networks"]["solana"]["expected_genesis_identity"]=serde_json::json!(bs58::encode([11;32]).into_string());
        let config=ValidatedConfig::from_effective_json(&json.to_string()).unwrap();
        for pool in &mut pools {pool.configuration_digest=config.digest().into();}
        let mut request=request(&config,&pools);request.network_id=NetworkId::SolanaMainnet;
        let traces=evaluate(&request,&gate()).unwrap();
        assert_eq!(traces.len(),4);
        assert_eq!(traces.iter().filter(|t|matches!(t.result,DecisionResult::Quoted {..})).count(),2);
        assert_eq!(traces.iter().filter(|t|matches!(&t.result,DecisionResult::Rejected {reason_codes} if reason_codes==&["UNSUPPORTED_SIZE"])).count(),2);
        for trace in traces {
            if let Some(opportunity)=trace.to_opportunity().unwrap() {
                assert_eq!(opportunity.evidence_label,arb_domain::Evidence::Candidate);
                assert_eq!(opportunity.dataset_origin,Some(DatasetOrigin::ManuallyConstructed));
                assert!(opportunity.net_after_explicit_costs_minor.is_none());
            }
        }
    }

}
