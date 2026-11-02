// Runs the pinned, unchanged upstream implementation; no expected results are supplied by Rust.
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
const repo=resolve(process.argv[2]??'../lattice-storage');
const pin='fc24f13fa4380aa32beb991749112429c682cf23';
if(execFileSync('git',['-C',repo,'rev-parse','HEAD'],{encoding:'utf8'}).trim()!==pin)throw Error('source pin mismatch');
if(execFileSync('git',['-C',repo,'status','--porcelain','--untracked-files=no'],{encoding:'utf8'}).trim())throw Error('modified source');
const sdk=await import(pathToFileURL(repo+'/vendor/lattice-sdk.js'));
const f=await import(pathToFileURL(repo+'/client/file.mjs'));
const hash=b=>createHash('sha256').update(b).digest('hex');
const bytes=n=>Uint8Array.from({length:n},(_,i)=>(i*31+7)&255);
const vol=b=>{const cid=sdk.cidV1DagCbor(b);return {cid,entries:{[cid]:Buffer.from(b).toString('base64')}}};
const value=v=>vol(sdk.encodeDagCbor(v));
const layouts=[];
for(const length of [0,1,23,24,255,256,65535,65536,f.CHUNK_BYTES-1,f.CHUNK_BYTES,f.CHUNK_BYTES+1,2*f.CHUNK_BYTES,2*f.CHUNK_BYTES+7])for(const type of ['application/octet-stream','😀'.repeat(127)+'x']){
 const input=bytes(length),vs=f.fileVolumes(sdk,type,input),root=f.readRoot(sdk,vs[0]);
 const chunks=vs.slice(1).map((v,i)=>f.readChunk(sdk,v,root,i));
 layouts.push({length,type,cids:vs.map(v=>v.cid),blockHashes:vs.map(v=>hash(Buffer.from(v.entries[v.cid],'base64'))),assembledHash:hash(f.assemble(root,chunks))});
}
const roots=[];
const add=(name,volume,maxBytes=null)=>{const r=f.readRoot(sdk,volume,maxBytes===null?{}:{maxBytes:BigInt(maxBytes)});roots.push({name,volume,maxBytes,block:f.readBlock(sdk,volume)!==undefined,root:r?{type:r.type,size:String(r.size),chunks:r.chunks??null,bytes:r.bytes?Buffer.from(r.bytes).toString('hex'):null}:null});};
const good=value({type:'x',size:3n,bytes:bytes(3)}),chunk=value({bytes:bytes(1)}).cid;
for(const [name,v]of Object.entries({good:{type:'x',size:3n,bytes:bytes(3)},empty:{type:'',size:0n,bytes:bytes(0)},negative:{type:'x',size:-1n,bytes:bytes(0)},wrongSize:{type:'x',size:4n,bytes:bytes(3)},missingType:{size:3n,bytes:bytes(3)},extra:{type:'x',size:3n,bytes:bytes(3),other:0n},longType:{type:'x'.repeat(256),size:0n,bytes:bytes(0)},unicodeType:{type:'😀'.repeat(128),size:0n,bytes:bytes(0)},boundaryType:{type:'😀'.repeat(127)+'x',size:0n,bytes:bytes(0)},chunks:{type:'x',size:1048577n,chunks:[chunk,chunk]},shortChunks:{type:'x',size:1048577n,chunks:[chunk]},smallChunked:{type:'x',size:1n,chunks:[chunk]},nonCid:{type:'x',size:1048577n,chunks:[chunk,'../pins']},uppercase:{type:'x',size:1048577n,chunks:[chunk,chunk.toUpperCase()]},wrongType:{type:1n,size:0n,bytes:bytes(0)},huge:{type:'x',size:18446744073709551615n,chunks:[chunk]},null:null,scalar:5n}))add(name,value(v));
add('limit-below',good,2);add('limit-equal',good,3);add('missing-entry',{cid:good.cid,entries:{}});add('wrong-bytes',{cid:good.cid,entries:{[good.cid]:chunk}});add('extra-entry',{cid:good.cid,entries:{...good.entries,invalid:'%%%'}});
for(const [name,encoded]of [['whitespace',' \n'+good.entries[good.cid]+'\t'],['unpadded',good.entries[good.cid].replace(/=+$/,'')],['invalid','%%%'],['overpadded',good.entries[good.cid]+'===']])add(name,{cid:good.cid,entries:{[good.cid]:encoded}});
for(const [name,hex]of [['noncanonical-int','1817'],['duplicate','a2616101616102'],['unsorted','a2616201616102'],['proto','a1695f5f70726f746f5f5f00'],['trailing','0000'],['float','fa3f800000'],['invalidutf8','61ff']])add(name,vol(Buffer.from(hex,'hex')));
const cidCases=[chunk,chunk.toUpperCase(),'','b','../pins',sdk.cidV1(0x55,bytes(1))].map(cid=>({cid,valid:f.isDagCborCID(sdk,cid)}));
writeFileSync('tests/fixtures/javascript.json',JSON.stringify({source:pin,sourceHashes:Object.fromEntries(['client/file.mjs','vendor/lattice-sdk.js'].map(p=>[p,hash(readFileSync(repo+'/'+p))])),layouts,roots,cidCases},null,2)+'\n');
console.log(`${layouts.length} layouts, ${roots.length} read cases, ${cidCases.length} CIDs`);
