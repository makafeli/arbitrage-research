use super::*;
use arb_domain::NetworkId;
use arb_paper::{InitialBalance, JournalEvent, PaperCommand, PaperRun, PortfolioBalance, PortfolioReservation};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewPaperRun { pub initial_balances: Vec<InitialBalance> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaperRunRecord {
 pub run_id: String, pub session_id: String, pub network_id: NetworkId, pub mode: String,
 pub configuration_digest: String, pub created_at: String, pub revision: String,
 pub initial_balances: Vec<InitialBalance>, pub balances: Vec<PortfolioBalance>,
 pub outstanding_reservations: u32, pub evidence_label: String, pub execution_authorized: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaperRunPage { pub items: Vec<PaperRunRecord>, pub next_cursor: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredPaperEvent { pub event_id: String, pub recorded_at: String, pub event: JournalEvent }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaperJournalPage { pub items: Vec<StoredPaperEvent>, pub next_cursor: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaperReservationPage { pub items: Vec<PortfolioReservation>, pub next_cursor: Option<String> }

impl Store {
 pub async fn replay_paper_run_creation(&self, operator: &str, session_id: &str, key: &str, input: &NewPaperRun) -> Result<Option<PaperRunRecord>, StoreError> {
  bounded(key,128,"invalid idempotency key")?;
  let mut tx=self.pool.begin().await?;
  let row=sqlx::query("SELECT k.payload_digest,r.* FROM paper_run_creation_keys k JOIN paper_runs r ON r.run_id=k.run_id WHERE k.operator_id=$1 AND k.session_id=$2 AND k.idempotency_key=$3 FOR SHARE OF r").bind(operator).bind(session_id).bind(key).fetch_optional(&mut *tx).await?;
  if let Some(row)=row { if row.try_get::<String,_>("payload_digest")?!=payload_digest(input)? { return Err(StoreError::Conflict("idempotency payload changed")); } Ok(Some(paper_projection(&mut tx,&row).await?)) } else { Ok(None) }
 }
 pub async fn create_paper_run(&self, operator: &str, session_id: &str, key: &str, input: NewPaperRun) -> Result<PaperRunRecord, StoreError> {
  bounded(key,128,"invalid idempotency key")?;
  if input.initial_balances.is_empty() || input.initial_balances.len()>32 { return Err(StoreError::InvalidInput("initial balance count must be 1..32")); }
  let digest=payload_digest(&input)?;
  let mut tx=self.pool.begin().await?;
  let session=sqlx::query("SELECT * FROM research_sessions WHERE operator_id=$1 AND session_id=$2 FOR UPDATE").bind(operator).bind(session_id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
  if let Some(row)=sqlx::query("SELECT k.payload_digest,r.* FROM paper_run_creation_keys k JOIN paper_runs r ON r.run_id=k.run_id WHERE k.operator_id=$1 AND k.session_id=$2 AND k.idempotency_key=$3").bind(operator).bind(session_id).bind(key).fetch_optional(&mut *tx).await? {
   if row.try_get::<String,_>("payload_digest")?!=digest { return Err(StoreError::Conflict("idempotency payload changed")); } return paper_projection(&mut tx,&row).await;
  }
  let state=lifecycle(&session)?;
  if state.mode()!=Mode::Paper { return Err(StoreError::CapabilityUnavailable); }
  let pending:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM control_commands WHERE session_id=$1 AND status='PENDING')").bind(session_id).fetch_one(&mut *tx).await?;
  if state.state()!=arb_domain::State::Stopped || pending { return Err(StoreError::Conflict("paper run creation requires a stopped session with no pending command")); }
  let network:NetworkId=session.try_get::<String,_>("network_id")?.parse().map_err(|_|StoreError::CorruptState)?;
  let config:String=session.try_get("configuration_digest")?;
  validate_paper_assets(&mut tx,operator,&config,network,&input.initial_balances).await?;
  let id=Uuid::new_v4().to_string();
  let run=PaperRun::new(&id,network,input.initial_balances.clone()).map_err(paper_input_error)?;
  let event=run.journal().first().ok_or(StoreError::CorruptState)?;
  let bytes=event_bytes(event)?;
  let row=sqlx::query("INSERT INTO paper_runs(run_id,operator_id,session_id,configuration_digest,network_id,initial_balances,journal_bytes) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING *").bind(&id).bind(operator).bind(session_id).bind(config).bind(network.as_str()).bind(serde_json::to_value(&input.initial_balances).map_err(|_|StoreError::CorruptState)?).bind(bytes).fetch_one(&mut *tx).await?;
  insert_paper_event(&mut tx,&id,event).await?;
  sqlx::query("INSERT INTO paper_run_creation_keys(operator_id,session_id,idempotency_key,payload_digest,run_id) VALUES($1,$2,$3,$4,$5)").bind(operator).bind(session_id).bind(key).bind(digest).bind(&id).execute(&mut *tx).await?;
  audit(&mut tx,session_id,operator,"PAPER_RUN_CREATED",None,json!({"run_id":id,"evidence":"HYPOTHETICAL","execution_authorized":false})).await?;
  let record=project_paper(&row,&run)?; tx.commit().await?; Ok(record)
 }
 pub async fn get_paper_run(&self,operator:&str,run_id:&str)->Result<PaperRunRecord,StoreError>{
  let mut tx=self.pool.begin().await?;
  let row=sqlx::query("SELECT * FROM paper_runs WHERE operator_id=$1 AND run_id=$2 FOR SHARE").bind(operator).bind(run_id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
  paper_projection(&mut tx,&row).await
 }
 pub async fn list_paper_runs(&self,operator:&str,session_id:&str,cursor:Option<&str>,limit:u32)->Result<PaperRunPage,StoreError>{
  validate_page(cursor,limit)?; self.get_session(operator,session_id).await?;
  let mut tx=self.pool.begin().await?;
  let rows=sqlx::query("SELECT * FROM paper_runs WHERE operator_id=$1 AND session_id=$2 AND ($3::text IS NULL OR run_id>$3) ORDER BY run_id LIMIT $4 FOR SHARE").bind(operator).bind(session_id).bind(cursor).bind(i64::from(limit)+1).fetch_all(&mut *tx).await?;
  let more=rows.len()>limit as usize; let mut items=Vec::new();
  for row in rows.iter().take(limit as usize) { items.push(paper_projection(&mut tx,row).await?); }
  let next_cursor=if more {items.last().map(|r|r.run_id.clone())}else{None}; Ok(PaperRunPage{items,next_cursor})
 }
 pub async fn list_paper_journal(&self,operator:&str,run_id:&str,cursor:Option<&str>,limit:u32)->Result<PaperJournalPage,StoreError>{
  if !(1..=100).contains(&limit){return Err(StoreError::InvalidInput("limit must be 1..100"));}
  let sequence=match cursor{Some(v)=>Some(parse_revision(v)? as i64),None=>None};
  let rows=sqlx::query("SELECT j.* FROM paper_journal j JOIN paper_runs r ON r.run_id=j.run_id WHERE r.operator_id=$1 AND r.run_id=$2 AND ($3::bigint IS NULL OR j.sequence>$3) ORDER BY j.sequence LIMIT $4").bind(operator).bind(run_id).bind(sequence).bind(i64::from(limit)+1).fetch_all(&self.pool).await?;
  if rows.is_empty(){self.get_paper_run(operator,run_id).await?;}
  let more=rows.len()>limit as usize;let items:Vec<_>=rows.iter().take(limit as usize).map(stored_paper_event).collect::<Result<_,_>>()?;
  let next_cursor=if more{items.last().map(|r|r.event.sequence().to_string())}else{None}; Ok(PaperJournalPage{items,next_cursor})
 }
 pub async fn list_paper_reservations(&self,operator:&str,run_id:&str,cursor:Option<&str>,limit:u32)->Result<PaperReservationPage,StoreError>{
  if !(1..=100).contains(&limit){return Err(StoreError::InvalidInput("limit must be 1..100"));} if let Some(c)=cursor{bounded(c,128,"invalid reservation cursor")?;}
  let mut tx=self.pool.begin().await?;
  let row=sqlx::query("SELECT * FROM paper_runs WHERE operator_id=$1 AND run_id=$2 FOR SHARE").bind(operator).bind(run_id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
  let run=restore_paper(&mut tx,&row).await?;
  let selected:Vec<_>=run.reservations().into_iter().filter(|r|cursor.is_none_or(|c|r.attempt_id.as_str()>c)).take(limit as usize+1).collect();
  let more=selected.len()>limit as usize;let items:Vec<_>=selected.into_iter().take(limit as usize).collect();
  let next_cursor=if more{items.last().map(|r|r.attempt_id.clone())}else{None};Ok(PaperReservationPage{items,next_cursor})
 }
 /// Internal hypothetical accounting only. No HTTP settlement endpoint or simulation claim.
 pub async fn apply_paper_command(&self,claim:&WorkerClaim,generation:u64,run_id:&str,key:&str,command:PaperCommand)->Result<StoredPaperEvent,StoreError>{
  bounded(key,128,"invalid paper command key")?;
  if matches!(command,PaperCommand::Initialize{..}){return Err(StoreError::InvalidInput("portfolio reset requires a new run"));}
  if serde_json::to_vec(&command).map_err(|_|StoreError::CorruptState)?.len()>32768{return Err(StoreError::InvalidInput("paper command exceeds size bound"));}
  let mut tx=self.pool.begin().await?;let session=worker::locked_worker(&mut tx,claim).await?;let state=lifecycle(&session)?;
  if state.mode()!=Mode::Paper{return Err(StoreError::CapabilityUnavailable);}
  let row=sqlx::query("SELECT * FROM paper_runs WHERE operator_id=$1 AND session_id=$2 AND run_id=$3 FOR UPDATE").bind(&claim.operator_id).bind(&claim.session_id).bind(run_id).fetch_optional(&mut *tx).await?.ok_or(StoreError::NotFound)?;
  if let Some(existing)=sqlx::query("SELECT * FROM paper_journal WHERE run_id=$1 AND command_id=$2").bind(run_id).bind(key).fetch_optional(&mut *tx).await?{
   let receipt=stored_paper_event(&existing)?;if receipt.event.command()!=&command{return Err(StoreError::Conflict("paper idempotency payload changed"));}return Ok(receipt);
  }
  if matches!(command,PaperCommand::Reserve{..}) && (!state.allows_evaluation() || state.generation()!=generation){return Err(StoreError::Conflict("paper reservation generation fenced"));}
  let mut run=restore_paper(&mut tx,&row).await?;let event=run.apply(key,command).map_err(paper_input_error)?;
  let total=row.try_get::<i64,_>("journal_bytes")?.checked_add(event_bytes(&event)?).ok_or(StoreError::CorruptState)?;
  if event.sequence()>4999 || total>16777216{return Err(StoreError::Conflict("paper run journal bound reached; create a new run"));}
  let receipt=insert_paper_event(&mut tx,run_id,&event).await?;
  sqlx::query("UPDATE paper_runs SET revision=$2,journal_bytes=$3 WHERE run_id=$1").bind(run_id).bind(event.sequence() as i64).bind(total).execute(&mut *tx).await?;
  audit(&mut tx,&claim.session_id,&claim.operator_id,"PAPER_JOURNAL_APPENDED",None,json!({"run_id":run_id,"event_id":receipt.event_id,"sequence":event.sequence().to_string(),"evidence":"HYPOTHETICAL"})).await?;
  tx.commit().await?;Ok(receipt)
 }
}
async fn validate_paper_assets(tx:&mut Transaction<'_,Postgres>,operator:&str,digest:&str,network:NetworkId,balances:&[InitialBalance])->Result<(),StoreError>{
 let snapshot:Value=sqlx::query_scalar("SELECT snapshot FROM configuration_snapshots WHERE operator_id=$1 AND configuration_digest=$2").bind(operator).bind(digest).fetch_one(&mut **tx).await?;
 let key=match network{NetworkId::BaseMainnet=>"base",NetworkId::SolanaMainnet=>"solana"};let configured=&snapshot["networks"][key];
 if snapshot["deployment"]["mode"]!="PAPER" || configured["enabled"]!=true{return Err(StoreError::InvalidInput("frozen paper network is not enabled"));}
 let allowed=configured["verified_asset_ids"].as_array().ok_or(StoreError::InvalidInput("frozen paper asset allowlist is missing"))?;
 for balance in balances{
  if balance.asset.network()!=network{return Err(StoreError::InvalidInput("paper balance has wrong network"));}
  if let arb_paper::AccountingAsset::Token(asset)=&balance.asset && !allowed.iter().any(|id|id.as_str()==Some(asset.to_string().as_str())){return Err(StoreError::InvalidInput("paper asset is not in frozen allowlist"));}
 } Ok(())
}
fn validate_page(cursor:Option<&str>,limit:u32)->Result<(),StoreError>{if !(1..=100).contains(&limit){return Err(StoreError::InvalidInput("limit must be 1..100"));}if let Some(c)=cursor{Uuid::parse_str(c).map_err(|_|StoreError::InvalidInput("invalid cursor"))?;}Ok(())}
fn paper_input_error(_:arb_paper::PaperError)->StoreError{StoreError::Conflict("paper command violates exact inventory or run invariants")}
fn event_bytes(event:&JournalEvent)->Result<i64,StoreError>{Ok(serde_json::to_vec(event).map_err(|_|StoreError::CorruptState)?.len() as i64)}
async fn insert_paper_event(tx:&mut Transaction<'_,Postgres>,run_id:&str,event:&JournalEvent)->Result<StoredPaperEvent,StoreError>{
 let row=sqlx::query("INSERT INTO paper_journal(event_id,run_id,sequence,command_id,payload_digest,payload) VALUES($1,$2,$3,$4,$5,$6) RETURNING *").bind(Uuid::new_v4().to_string()).bind(run_id).bind(event.sequence() as i64).bind(event.command_id()).bind(payload_digest(event)?).bind(serde_json::to_value(event).map_err(|_|StoreError::CorruptState)?).fetch_one(&mut **tx).await?;stored_paper_event(&row)
}
fn stored_paper_event(row:&PgRow)->Result<StoredPaperEvent,StoreError>{
 let event:JournalEvent=serde_json::from_value(row.try_get("payload")?).map_err(|_|StoreError::CorruptState)?;
 if payload_digest(&event)?!=row.try_get::<String,_>("payload_digest")? || event.run_id()!=row.try_get::<String,_>("run_id")? || event.command_id()!=row.try_get::<String,_>("command_id")? || event.sequence()!=row.try_get::<i64,_>("sequence")? as u64{return Err(StoreError::CorruptState);}
 Ok(StoredPaperEvent{event_id:row.try_get("event_id")?,recorded_at:row.try_get::<DateTime<Utc>,_>("recorded_at")?.to_rfc3339(),event})
}
async fn restore_paper(tx:&mut Transaction<'_,Postgres>,row:&PgRow)->Result<PaperRun,StoreError>{
 let rows=sqlx::query("SELECT * FROM paper_journal WHERE run_id=$1 ORDER BY sequence LIMIT 5001").bind(row.try_get::<String,_>("run_id")?).fetch_all(&mut **tx).await?;
 if rows.len()>5000 || rows.len()!=row.try_get::<i64,_>("revision")? as usize+1{return Err(StoreError::CorruptState);}
 let events:Vec<_>=rows.iter().map(stored_paper_event).collect::<Result<Vec<_>,_>>()?.into_iter().map(|r|r.event).collect();
 let bytes=events.iter().try_fold(0_i64,|sum,event|sum.checked_add(event_bytes(event)?).ok_or(StoreError::CorruptState))?;
 if bytes!=row.try_get::<i64,_>("journal_bytes")?{return Err(StoreError::CorruptState);}
 let run=PaperRun::replay(&events).map_err(|_|StoreError::CorruptState)?;
 let initial:Vec<InitialBalance>=serde_json::from_value(row.try_get("initial_balances")?).map_err(|_|StoreError::CorruptState)?;
 if events.first().map(JournalEvent::command)!=Some(&PaperCommand::Initialize{balances:initial}) || events.first().ok_or(StoreError::CorruptState)?.network().as_str()!=row.try_get::<String,_>("network_id")?{return Err(StoreError::CorruptState);}Ok(run)
}
async fn paper_projection(tx:&mut Transaction<'_,Postgres>,row:&PgRow)->Result<PaperRunRecord,StoreError>{let run=restore_paper(tx,row).await?;project_paper(row,&run)}
fn project_paper(row:&PgRow,run:&PaperRun)->Result<PaperRunRecord,StoreError>{Ok(PaperRunRecord{
 run_id:row.try_get("run_id")?,session_id:row.try_get("session_id")?,network_id:row.try_get::<String,_>("network_id")?.parse().map_err(|_|StoreError::CorruptState)?,mode:"PAPER".into(),configuration_digest:row.try_get("configuration_digest")?,created_at:row.try_get::<DateTime<Utc>,_>("created_at")?.to_rfc3339(),revision:row.try_get::<i64,_>("revision")?.to_string(),initial_balances:serde_json::from_value(row.try_get("initial_balances")?).map_err(|_|StoreError::CorruptState)?,balances:run.balances().map_err(|_|StoreError::CorruptState)?,outstanding_reservations:u32::try_from(run.outstanding_reservations()).map_err(|_|StoreError::CorruptState)?,evidence_label:"HYPOTHETICAL".into(),execution_authorized:false
})}
