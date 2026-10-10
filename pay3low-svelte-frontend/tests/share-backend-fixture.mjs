import {createServer} from 'node:http';
import {createHash} from 'node:crypto';
const shares=new Map();
const server=createServer(async(req,res)=>{
 if(req.url==='/health'){res.end('ok');return;}
 res.setHeader('content-type','application/json');
 if(req.method==='POST' && req.url==='/api/share-links'){
  let raw='';for await(const chunk of req)raw+=chunk;
  const data=JSON.parse(raw);
  if(typeof data.target!=='string'){res.writeHead(400);res.end('{}');return;}
  const id=createHash('sha256').update(data.target+JSON.stringify(data.preview??null)).digest().subarray(0,12).toString('base64url');
  const stored={id,target:data.target,preview:data.preview??null};shares.set(id,stored);
  res.end(JSON.stringify(stored));return;
 }
 const id=req.url?.split('/').at(-1),data=shares.get(id);
 if(!data){res.writeHead(404);res.end('{}');return;}
 res.end(JSON.stringify(data));
});
server.listen(Number(process.env.PLAYWRIGHT_SHARE_PORT ?? 8091),'127.0.0.1',()=>console.log('Share backend fixture ready'));
process.on('SIGTERM', () => server.close());
