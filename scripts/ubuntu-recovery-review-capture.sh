#!/usr/bin/env bash
set -euo pipefail
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
umask 077
mkdir -p var/recovery-review-caps/capture
source scripts/ubuntu-env.sh
GIT_MASTER=1 git rev-parse HEAD > var/recovery-review-caps/capture/source-head.txt
node --input-type=module -e 'import {readFile,access,cp,writeFile} from "node:fs/promises"; const log=await readFile("var/recovery-review-caps/original.log","utf8"); const roots=[...new Set([...log.matchAll(/(\/tmp\/\.tmp[A-Za-z0-9]+)\/[0-9a-f-]+\/segment-\d+\.mkv/g)].map(v=>v[1]))]; const results=[]; for(const [index,source] of roots.entries()){try{await access(source); const destination=`var/recovery-review-caps/capture/original-timeout-${index}`; await cp(source,destination,{recursive:true,errorOnExist:true,force:false}); results.push({source,destination,captured:true});}catch(error){if(error.code!=="ENOENT")throw error;results.push({source,captured:false,reason:"notPresentAfterReboot"});}} await writeFile("var/recovery-review-caps/capture/original-timeout.json",JSON.stringify({snapshotPolicy:"owned test roots parsed from its private trace; not decode promotion",results},null,2),{mode:0o600}); console.log(JSON.stringify(results));'
ps -eo pid,ppid,stat,comm > var/recovery-review-caps/capture/processes-after.txt
