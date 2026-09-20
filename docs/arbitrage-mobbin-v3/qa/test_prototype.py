"""Browser checks for the standalone, synthetic UI prototype; no production calls.
Requires Python, Playwright and Chromium. Set CHROMIUM_PATH when needed.
The document is injected into an isolated page to avoid network navigation.
"""
from pathlib import Path
import json, os
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[1]
html=(ROOT/'index.html').read_text().replace('<link rel="stylesheet" href="assets/styles.css">','<style>'+ (ROOT/'assets/styles.css').read_text()+'</style>').replace('<script src="assets/app.js"></script>','<script>'+ (ROOT/'assets/app.js').read_text()+'</script>')
results={'checks':[], 'layout':[], 'page_errors':[], 'external_requests':[]}
def check(name, condition, detail=''):
 results['checks'].append({'name':name,'pass':bool(condition),'detail':detail})
 if not condition: print('FAILED',name,detail)
with sync_playwright() as w:
 b=w.chromium.launch(executable_path=os.environ.get('CHROMIUM_PATH','/usr/bin/chromium'),args=['--no-sandbox'])
 page=b.new_page(viewport={'width':1440,'height':1000},device_scale_factor=1,accept_downloads=True)
 page.on('pageerror',lambda e:results['page_errors'].append(str(e)))
 page.on('request',lambda r:results['external_requests'].append(r.url) if r.url.startswith(('http:','https:')) else None)
 page.set_content(html,wait_until='load')
 def route(name,tab=''):
  page.evaluate('([name,tab])=>{state.scenario="snapshot";state.chain="all";state.query="";state.evidence="all";navigate(name,tab)}',[name,tab]);page.wait_for_timeout(30)
 def close():
  if page.locator('#dialog').evaluate('(e)=>e.open'):
   page.keyboard.press('Escape'); page.wait_for_timeout(100)
 check('Light is the first-load default', page.evaluate('document.documentElement.dataset.theme')=='light')
 check('Primary action follows the ink palette', page.locator('.head-actions .primary').evaluate('(e)=>getComputedStyle(e).backgroundColor')=='rgb(20, 20, 20)')
 # Real UI navigation and inspection
 page.locator('.primary-pill a[href="#/opportunities"]').click()
 page.wait_for_function('state.page === "opportunities"')
 check('Primary navigation changes page',page.locator('h1').inner_text()=='Opportunities.')
 page.locator('#record-search').fill('not-a-match')
 check('Record search empty state',page.get_by_text('No captured records in this view',exact=True).is_visible())
 page.locator('#record-search').fill('USDC')
 page.locator('.desktop-records [data-action="record"]').click()
 check('Record detail leads with rejection and unknown net', 'simulation failed' in page.locator('#dialog').inner_text().lower() and 'unknown' in page.locator('#dialog').inner_text().lower())
 page.keyboard.press('Escape');page.wait_for_timeout(100)
 check('Escape returns focus to the record control',page.evaluate('document.activeElement?.dataset.action')=='record')
 page.keyboard.press('Control+k');page.locator('#global-search').fill('system')
 check('Global search works',page.locator('#search-results').inner_text().lower().find('system')>=0)
 close()
 page.locator('#tab-captured').focus();page.keyboard.press('ArrowRight')
 check('Tabs support arrow keys',page.locator('#tab-decisions').get_attribute('aria-selected')=='true')
 # New view-switch interaction on the same evidence.
 route('opportunities','captured')
 page.locator('#record-search').fill('USDC')
 page.locator('[data-layout="cards"]').click()
 check('Card layout uses the same filtered record',page.locator('.evidence-tile').count()==1 and page.evaluate('state.query')=='USDC')
 check('Card layout retains an unknown net and rejection', 'Unknown' in page.locator('.evidence-tile').inner_text() and 'Rejected' in page.locator('.evidence-tile').inner_text())
 page.locator('.evidence-tile [data-action="record"]').click()
 check('Card opens the same evidence inspector', page.evaluate('state.selected')=='record-1')
 close()
 page.locator('[data-layout="list"]').click()
 check('List switch preserves the query', page.locator('#record-search').input_value()=='USDC')
 # Request != application; drain != stopped
 route('runs')
 page.locator('[data-action="session"][data-id="session-paper"]').first.click()
 page.locator('[data-action="session-command"][data-command="START"]').click()
 page.locator('[data-action="confirm-command"]').click()
 check('Request stays pending without changing observed state',page.evaluate('state.receipts["session-paper"].status==="PENDING" && state.sessions[0].state==="STOPPED"'))
 page.locator('[data-action="ack"]').click()
 check('Only explicit simulated acknowledgement applies start',page.evaluate('state.receipts["session-paper"].status==="APPLIED" && state.sessions[0].state==="RUNNING"'))
 close()
 page.locator('[data-action="session"][data-id="session-solana-draining"]').first.click()
 check('Draining retains unresolved outcomes',page.evaluate('state.sessions[2].state==="DRAINING" && state.sessions[2].unresolved===2'))
 page.locator('[data-action="reconcile"]').click()
 check('Separate reconciliation produces stopped',page.evaluate('state.sessions[2].state==="STOPPED" && state.sessions[2].unresolved===0'))
 close()
 page.evaluate('state.sessions=freshSessions();state.receipts={};render()')
 # View filter does not narrow bulk-control scope
 page.locator('#chain-filter').select_option('base')
 page.locator('[data-action="stop-all"]').click()
 text=page.locator('#dialog').inner_text()
 check('Bulk stop enumerates all loaded scope despite chain filter','session-solana-draining' in text and 'session-base-running' in text)
 close()
 # Form gating, immutable mode, create in recovery
 route('experiments')
 check('Create requires an acknowledgement and reference',page.locator('#create-session').is_disabled())
 page.locator('#experiment-reference').fill('Design QA session');page.locator('#experiment-review').check()
 page.locator('#create-session').click()
 check('New session never auto-starts',page.evaluate('state.sessions.at(-1).state==="RECOVERING"'))
 close()
 # Exact integers and missing != zero
 route('opportunities','costs')
 check('Incomplete costs produce Unknown',page.locator('#cost-result').inner_text()=='Unknown')
 for i in range(6):page.locator(f'#cost-kind-{i}').select_option('zero')
 check('Explicit zero costs are different from missing',page.locator('#cost-result').inner_text()=='+10')
 page.locator('#cost-kind-0').select_option('known');page.locator('#cost-value-0').fill('900719925474099300000')
 check('Arbitrary-size integer arithmetic preserves precision',page.locator('#cost-result').inner_text()==str(10-900719925474099300000))
 page.locator('#cost-value-0').fill('1.5')
 check('Fractional minor units remain invalid',page.locator('#cost-result').inner_text()=='Unknown' and page.locator('#save-cost').is_disabled())
 page.locator('#cost-value-0').fill('6');page.locator('#cost-review').check();page.locator('#save-cost').click()
 check('Saving assumption does not change source quote',page.evaluate('state.assessment.net==="4" && decisions[0].gross==="10"'))
 route('opportunities','exports');page.locator('[data-action="freeze"]').click()
 frozen=page.evaluate('JSON.stringify(state.frozen)')
 page.evaluate('state.assessment.net="-9"')
 check('Frozen export isolated from later model changes',page.evaluate('JSON.stringify(state.frozen)')==frozen)
 with page.expect_download() as dl:page.locator('[data-action="export-json"]').click()
 artifact=dl.value;data=json.loads(Path(artifact.path()).read_text())
 check('Download is a labelled synthetic frozen JSON object',data['kind']=='SYNTHETIC_DESIGN_EXPORT' and data['costAssessments'][0]['net']=='4')
 with page.expect_download() as dl:page.locator('[data-action="export-csv"]').click()
 csvtext=Path(dl.value.path()).read_text()
 check('CSV contains exact integers and unknown net','100000000' in csvtext and 'UNKNOWN' in csvtext)
 # Account preview and locked live workspace
 route('signin');page.locator('[data-action="signin-help"]').click()
 check('Recovery does not claim an email was sent','no reset email has been sent' in page.locator('#signin-help').inner_text())
 route('real');check('Real execution remains disabled',page.locator('button:disabled').count()>0 and 'not available' in page.locator('main').inner_text())
 # Reset for consistent layout checks and previews
 page.evaluate('state.sessions=freshSessions();state.receipts={};state.assessment=null;state.costs=costNames.map(()=>({kind:"missing",value:""}));state.costReview=false;state.form={config:"base-paper",reference:"",review:false};state.frozen=null')
 pages=[('overview',''),('opportunities','captured'),('opportunities','decisions'),('opportunities','costs'),('opportunities','exports'),('experiments',''),('runs','sessions'),('runs','ledger'),('runs','journal'),('runs','reservations'),('strategies',''),('system','health'),('system','collection'),('system','capabilities'),('real',''),('signin','')]
 for theme in ['dark','light']:
  page.evaluate('(t)=>document.documentElement.dataset.theme=t',theme)
  for width in [320,390,768,1024,1280,1440]:
   page.set_viewport_size({'width':width,'height':844 if width<760 else 1000})
   for name,tab in pages:
    route(name,tab)
    v=page.evaluate('({width:innerWidth,scroll:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight})')
    results['layout'].append({'theme':theme,'width':width,'page':name,'tab':tab,**v,'pass':v['scroll']<=width+1})
    if v['scroll']>width+1:print('OVERFLOW',theme,width,name,tab,v)
 # Gallery layout and modal checks.
 for theme in ['dark','light']:
  page.evaluate('(t)=>document.documentElement.dataset.theme=t',theme)
  for width in [320,390,768,1024,1280,1440]:
   page.set_viewport_size({'width':width,'height':844 if width<760 else 1000})
   route('opportunities','captured');page.locator('[data-layout="cards"]').click()
   v=page.evaluate('({width:innerWidth,scroll:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight})')
   results['layout'].append({'theme':theme,'width':width,'page':'opportunities','tab':'cards',**v,'pass':v['scroll']<=width+1})
   page.locator('.evidence-tile [data-action="record"]').click()
   check(f'Inspector fits the {theme} {width}px viewport', page.locator('#dialog').evaluate('(e)=>{const r=e.getBoundingClientRect();return r.width<=innerWidth+1&&r.height<=innerHeight+1&&r.x>=-1&&r.y>=-1}'))
   close();page.locator('[data-layout="list"]').click()
 # Screenshots, dark/light and desktop/mobile
 for theme in ['dark','light']:
  page.evaluate('(t)=>document.documentElement.dataset.theme=t',theme)
  for device,width,height in [('desktop',1440,1000),('mobile',390,844)]:
   page.set_viewport_size({'width':width,'height':height})
   for name,tab in [('overview',''),('opportunities','captured'),('opportunities','costs'),('experiments',''),('runs','sessions'),('signin','')]:
    # Both themes captured.
    route(name,tab);suffix=('-'+tab) if tab else ''
    page.screenshot(path=str(ROOT/'previews'/f'{name}{suffix}-{theme}-{device}.png'),full_page=True)
    if name=='overview':page.screenshot(path=str(ROOT/'previews'/f'overview-{theme}-{device}-viewport.png'))
   if True:
    route('opportunities')
    page.locator(('.desktop-records' if device=='desktop' else '.mobile-records')+' [data-action="record"]').first.click()
    page.screenshot(path=str(ROOT/'previews'/f'evidence-detail-{theme}-{device}.png'))
    close()
 # New gallery previews.
 for theme in ['light','dark']:
  page.evaluate('(t)=>document.documentElement.dataset.theme=t',theme)
  for device,width,height in [('desktop',1440,1000),('mobile',390,844)]:
   page.set_viewport_size({'width':width,'height':height})
   route('opportunities','captured');page.locator('[data-layout="cards"]').click()
   page.screenshot(path=str(ROOT/'previews'/f'opportunities-cards-{theme}-{device}.png'),full_page=True)
   page.locator('[data-layout="list"]').click()
 # State views
 page.set_viewport_size({'width':1440,'height':1000});page.evaluate('document.documentElement.dataset.theme="dark"')
 for scenario in ['stale','error','empty','loading']:
  route('overview');page.evaluate('(s)=>{state.scenario=s;render()}',scenario)
  check('Overview preview state: '+scenario, bool(page.locator('main').inner_text().strip()))
  page.screenshot(path=str(ROOT/'previews'/f'overview-{scenario}-desktop.png'),full_page=True)
 check('No JavaScript runtime errors',not results['page_errors'],str(results['page_errors']))
 check('No external requests',not results['external_requests'],str(results['external_requests']))
 check('No page-level horizontal overflow across tested layouts',all(v['pass'] for v in results['layout']),str(len(results['layout']))+' layout combinations')
 b.close()
results['summary']={'checks':len(results['checks']),'passed':sum(x['pass'] for x in results['checks']),'layout_combinations':len(results['layout']),'layout_passed':sum(x['pass'] for x in results['layout'])}
(ROOT/'qa'/'results.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results['summary']))
