import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const target=path.resolve(process.argv[2]??'');
if(path.dirname(target)!==path.join(repo,'logs')||!/^check-temp-[0-9a-f]{32}$/.test(path.basename(target)))throw new Error('Unexpected test cleanup boundary');
if(fs.realpathSync(path.join(repo,'logs'))!==path.join(fs.realpathSync(repo),'logs'))throw new Error('Test parent must not redirect outside the repository');
function remove(dir){
 const stat=fs.lstatSync(dir);
 if(stat.isSymbolicLink()){
  if(dir===target)throw new Error('Test root must not be a link: '+dir);
  fs.unlinkSync(dir);return;
 }
 if(stat.isDirectory()){
  for(const name of fs.readdirSync(dir))remove(path.join(dir,name));
  fs.rmdirSync(dir);
 }else fs.unlinkSync(dir);
}
if(fs.existsSync(target))remove(target);
if(fs.existsSync(target))throw new Error('Test directory still exists after cleanup: '+target);
console.log('Disposed isolated test directory: '+target);
