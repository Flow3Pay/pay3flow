from pathlib import Path
import subprocess, re
# Uses only the explicitly named disposable test container, never production.
import os
root=Path(__file__).resolve().parents[2]
container=os.environ.get('PAY3FLOW_TEST_POSTGRES_CONTAINER','pay3flow-georgia-regression')
marker=subprocess.run(['docker','inspect','--format','{{ index .Config.Labels "pay3flow.test.fixture" }}',container],text=True,capture_output=True,check=True).stdout.strip()
if marker != 'b2b-polling': raise RuntimeError('refusing an unmarked database container')
def sql(text):
    p=subprocess.run(['docker','exec','-i',container,'psql','-U','postgres','-d','pay3flow_test','-v','ON_ERROR_STOP=1','-Atq'],input=text,text=True,capture_output=True)
    if p.returncode: raise RuntimeError(p.stderr)
    return [line.split('|') for line in p.stdout.strip().splitlines() if line]
if sql('SELECT current_database()') != [['pay3flow_test']]: raise RuntimeError('refusing non-test database')
source=(root/'backend/src/b2b/mod.rs').read_text().split('async fn accepted_orders(&self)',1)[1]
query=re.search(r'r#"(.*?)"#',source,re.S).group(1)
sql('DROP SCHEMA public CASCADE; CREATE SCHEMA public; CREATE TABLE users (id UUID PRIMARY KEY);'+(root/'backend/migrations/b2b.sql').read_text()+(root/'backend/migrations/b2b_api_keys.sql').read_text())
sql("INSERT INTO users VALUES ('00000000-0000-4000-8000-000000000001'); INSERT INTO b2b_merchants(id,user_id,name) VALUES ('00000000-0000-4000-8000-000000000002','00000000-0000-4000-8000-000000000001','Test');")
def seed(extra):
    sql("TRUNCATE b2b_orders CASCADE; INSERT INTO b2b_orders(merchant_id,env,kind,input_asset,input_network,output_asset,output_network,recipient,status,created_at,updated_at) SELECT '00000000-0000-4000-8000-000000000002','test','checkout','USDT','ethereum','USDT','tron','test','awaiting_deposit',now()+i*interval '1 second',now()+i*interval '1 second' FROM generate_series(1,51) i;"+extra+" INSERT INTO b2b_quotes(order_id,provider,provider_quote_id,exact,input_amount,output_amount,best_output,deposit_address,refund_address,expires_at,raw) SELECT id,'test',id::text,'input','10','9','9','test','test',now()+interval '5 minutes','{}' FROM b2b_orders; UPDATE b2b_orders o SET accepted_quote_id=q.id FROM b2b_quotes q WHERE q.order_id=o.id;")
errors=[]
seed('')
first=sql(query);second=sql(query)
if len({r[0] for r in first+second}) != 51: errors.append('polling window starves order 51')
seed(" INSERT INTO b2b_orders(merchant_id,env,kind,input_asset,input_network,output_asset,output_network,recipient,status,updated_at) SELECT '00000000-0000-4000-8000-000000000002','test','checkout','USDT','ethereum','USDT','tron','test',s,now()-interval '1 day' FROM unnest(ARRAY['underpaid','overpaid','expired','cancelled','failed','paid','refunded']) s;")
statuses=[r[2] for r in sql(query)]
if not {'underpaid','expired','cancelled','failed'} <= set(statuses): errors.append('unsettled issued addresses lose observation')
if {'paid','overpaid','refunded'} & set(statuses): errors.append('completed orders remain in poll window')
if errors: raise AssertionError('; '.join(errors))
print('PASS: actual accepted_orders SQL advances across 51 orders and continues refund observation and excludes completed orders')
