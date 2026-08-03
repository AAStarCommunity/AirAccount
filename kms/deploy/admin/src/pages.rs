//! 内嵌页面(增量 1:登录 + 面板)。纯自包含,无外部资源(CSP default-src 'none';
//! 仅内联 <style>/<script>)。深浅色双主题。其余屏(apply 确认 / 进度 SSE / 审计)增量 2。

pub const LOGIN: &str = r##"<!doctype html>
<html lang="zh"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>AAStar 节点管理台 · 登录</title>
<style>
:root{--bg:#f4f5f7;--card:#fff;--fg:#1a1d24;--muted:#5b6472;--line:#d9dee6;--accent:#2f6f4f;--accent-fg:#fff;--err:#b3261e}
@media(prefers-color-scheme:dark){:root{--bg:#0f1216;--card:#171b21;--fg:#e6e9ee;--muted:#98a2b3;--line:#2a313b;--accent:#3f9d70;--accent-fg:#08130d;--err:#f2b8b5}}
:root[data-theme=light]{--bg:#f4f5f7;--card:#fff;--fg:#1a1d24;--muted:#5b6472;--line:#d9dee6;--accent:#2f6f4f;--accent-fg:#fff;--err:#b3261e}
:root[data-theme=dark]{--bg:#0f1216;--card:#171b21;--fg:#e6e9ee;--muted:#98a2b3;--line:#2a313b;--accent:#3f9d70;--accent-fg:#08130d;--err:#f2b8b5}
*{box-sizing:border-box}
body{margin:0;min-height:100vh;display:grid;place-items:center;background:var(--bg);color:var(--fg);
 font:15px/1.5 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}
.card{width:min(92vw,380px);background:var(--card);border:1px solid var(--line);border-radius:12px;padding:28px}
h1{font-size:17px;margin:0 0 4px}
.sub{color:var(--muted);font-size:13px;margin:0 0 20px}
label{display:block;font-size:12px;color:var(--muted);margin:14px 0 6px;letter-spacing:.02em}
input{width:100%;padding:10px 12px;border:1px solid var(--line);border-radius:8px;background:var(--bg);color:var(--fg);font-size:15px}
input:focus{outline:2px solid var(--accent);outline-offset:1px}
button{width:100%;margin-top:20px;padding:11px;border:0;border-radius:8px;background:var(--accent);color:var(--accent-fg);
 font-size:15px;font-weight:600;cursor:pointer}
button:disabled{opacity:.6;cursor:progress}
.msg{margin-top:14px;font-size:13px;color:var(--err);min-height:1.2em}
.foot{margin-top:18px;font-size:11px;color:var(--muted);text-align:center}
</style></head><body>
<main class="card">
 <h1>节点更新管理台</h1>
 <p class="sub">仅本机 / Tailscale 可访问 · 密码登录</p>
 <form id="f" autocomplete="off">
  <label for="pw">管理员密码</label>
  <input id="pw" type="password" required autofocus>
  <button id="b" type="submit">登录</button>
  <div class="msg" id="m"></div>
 </form>
 <div class="foot">AirAccount · aastar-node-updater</div>
</main>
<script>
const f=document.getElementById('f'),b=document.getElementById('b'),m=document.getElementById('m');
f.addEventListener('submit',async e=>{
 e.preventDefault();b.disabled=true;m.textContent='';
 try{
  const r=await fetch('/api/login',{method:'POST',headers:{'Content-Type':'application/json'},
   body:JSON.stringify({password:document.getElementById('pw').value})});
  const j=await r.json();
  if(r.ok){sessionStorage.setItem('csrf',j.csrf);location.href='/dashboard';}
  else{m.textContent=j.error||'登录失败';}
 }catch(err){m.textContent='网络错误';}
 finally{b.disabled=false;}
});
</script></body></html>"##;

pub const DASHBOARD: &str = r##"<!doctype html>
<html lang="zh"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>AAStar 节点管理台</title>
<style>
:root{--bg:#f4f5f7;--card:#fff;--fg:#1a1d24;--muted:#5b6472;--line:#d9dee6;--accent:#2f6f4f;--accent-fg:#fff;--ok:#2f6f4f;--warn:#8a6d1a;--err:#b3261e}
@media(prefers-color-scheme:dark){:root{--bg:#0f1216;--card:#171b21;--fg:#e6e9ee;--muted:#98a2b3;--line:#2a313b;--accent:#3f9d70;--accent-fg:#08130d;--ok:#3f9d70;--warn:#d6b24a;--err:#f2b8b5}}
:root[data-theme=light]{--bg:#f4f5f7;--card:#fff;--fg:#1a1d24;--muted:#5b6472;--line:#d9dee6;--accent:#2f6f4f;--accent-fg:#fff;--ok:#2f6f4f;--warn:#8a6d1a;--err:#b3261e}
:root[data-theme=dark]{--bg:#0f1216;--card:#171b21;--fg:#e6e9ee;--muted:#98a2b3;--line:#2a313b;--accent:#3f9d70;--accent-fg:#08130d;--ok:#3f9d70;--warn:#d6b24a;--err:#f2b8b5}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.55 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif}
header{display:flex;align-items:center;justify-content:space-between;padding:16px 22px;border-bottom:1px solid var(--line)}
header h1{font-size:15px;margin:0}
header button{background:none;border:1px solid var(--line);color:var(--muted);border-radius:7px;padding:6px 12px;font-size:13px;cursor:pointer}
main{max-width:820px;margin:0 auto;padding:22px}
.card{background:var(--card);border:1px solid var(--line);border-radius:12px;padding:20px;margin-bottom:18px}
.card h2{font-size:13px;text-transform:uppercase;letter-spacing:.05em;color:var(--muted);margin:0 0 14px}
.row{display:flex;justify-content:space-between;padding:7px 0;border-bottom:1px solid var(--line);font-size:14px}
.row:last-child{border-bottom:0}
.row b{font-weight:600}
.pill{display:inline-block;padding:2px 9px;border-radius:999px;font-size:12px;font-weight:600}
.pill.ok{background:color-mix(in srgb,var(--ok) 18%,transparent);color:var(--ok)}
.pill.warn{background:color-mix(in srgb,var(--warn) 20%,transparent);color:var(--warn)}
.pill.err{background:color-mix(in srgb,var(--err) 18%,transparent);color:var(--err)}
pre{background:var(--bg);border:1px solid var(--line);border-radius:8px;padding:12px;overflow-x:auto;font-size:12px;margin:0}
.btns{display:flex;gap:10px;flex-wrap:wrap;margin-top:6px}
button.act{border:0;border-radius:8px;padding:10px 16px;font-size:14px;font-weight:600;cursor:pointer;background:var(--accent);color:var(--accent-fg)}
button.ghost{background:none;border:1px solid var(--line);color:var(--fg)}
.muted{color:var(--muted);font-size:13px}
dialog{border:1px solid var(--line);border-radius:12px;background:var(--card);color:var(--fg);padding:22px;width:min(92vw,360px)}
dialog input{width:100%;padding:9px 11px;margin-top:8px;border:1px solid var(--line);border-radius:7px;background:var(--bg);color:var(--fg);font-size:15px}
dialog::backdrop{background:rgba(0,0,0,.45)}
</style></head><body>
<header><h1>节点更新管理台</h1><button onclick="logout()">登出</button></header>
<main>
 <div class="card"><h2>节点状态</h2><div id="status"><span class="muted">加载中…</span></div></div>
 <div class="card"><h2>可用更新</h2>
  <div id="cands"><span class="muted">点击「检查更新」拉取签名清单</span></div>
  <div class="btns"><button class="act ghost" onclick="check()">检查更新</button></div>
 </div>
 <div class="card"><h2>操作</h2>
  <p class="muted">应用更新 / 回滚均需 Telegram 二次确认;含 TA 变更的更新由 updater 拒绝(需人工现场)。</p>
  <div class="btns">
   <button class="act" onclick="askApply()">应用更新…</button>
   <button class="act ghost" onclick="askRollback()">回滚到上一个正常版本…</button>
  </div>
  <pre id="log" style="margin-top:14px;display:none"></pre>
 </div>
</main>
<dialog id="dlg"><form method="dialog">
 <div id="dlgbody"></div>
 <div class="btns" style="margin-top:16px"><button value="ok" class="act">确认</button><button value="cancel" class="act ghost">取消</button></div>
</form></dialog>
<script>
const csrf=()=>sessionStorage.getItem('csrf')||'';
function j(url,opt){opt=opt||{};opt.headers=Object.assign({'Content-Type':'application/json','X-CSRF-Token':csrf()},opt.headers||{});return fetch(url,opt).then(async r=>{const b=await r.json().catch(()=>({}));if(r.status===401){location.href='/';}return {ok:r.ok,b};});}
function pill(ok){return ok?'<span class="pill ok">正常</span>':'<span class="pill err">异常</span>';}
async function loadStatus(){
 const {ok,b}=await j('/api/status');
 const el=document.getElementById('status');
 if(!ok){el.innerHTML='<span class="pill err">读取失败</span>';return;}
 const s=b.state||{};
 el.innerHTML=`<div class="row"><span>节点</span><b>${b.node||'-'}</b></div>`+
  `<div class="row"><span>当前版本</span><b>${s.version||s.current||'-'}</b></div>`+
  `<div class="row"><span>updater 状态</span>${pill(b.ok)}</div>`;
}
async function check(){
 document.getElementById('cands').innerHTML='<span class="muted">检查中…</span>';
 const {ok,b}=await j('/api/candidates');
 document.getElementById('cands').innerHTML=ok
  ?`<pre>${(b.log||'(无输出)').replace(/[<&]/g,c=>c==='<'?'&lt;':'&amp;')}</pre><p class="muted">${b.note||''}</p>`
  :'<span class="pill err">检查失败</span>';
 loadStatus();
}
function dialog(html){return new Promise(res=>{const d=document.getElementById('dlg');document.getElementById('dlgbody').innerHTML=html;d.onclose=()=>res(d.returnValue);d.showModal();});}
async function askApply(){
 const v=await dialog('<h3 style="margin:0 0 6px">应用更新</h3><label class="muted">目标版本 (x.y.z)</label><input id="ver" placeholder="0.29.1" autofocus>');
 if(v!=='ok')return;const ver=document.getElementById('ver')?.value?.trim();if(!ver)return;
 const {ok,b}=await j('/api/apply',{method:'POST',body:JSON.stringify({version:ver})});
 if(!ok){showLog(b.error||'发起失败');return;}
 await confirm2fa(b.challenge_id);
}
async function askRollback(){
 const v=await dialog('<h3 style="margin:0 0 6px">回滚</h3><p class="muted">回滚到上一个健康版本?将发送 Telegram 确认码。</p>');
 if(v!=='ok')return;
 const {ok,b}=await j('/api/rollback',{method:'POST',body:JSON.stringify({})});
 if(!ok){showLog(b.error||'发起失败');return;}
 await confirm2fa(b.challenge_id);
}
async function confirm2fa(cid){
 const v=await dialog('<h3 style="margin:0 0 6px">Telegram 二次确认</h3><label class="muted">已发确认码到 Telegram,请回填(8 位)</label><input id="code" inputmode="numeric" placeholder="8 位码" autofocus>');
 if(v!=='ok')return;const code=document.getElementById('code')?.value?.trim();if(!code)return;
 showLog('执行中…');
 const {ok,b}=await j('/api/apply/confirm',{method:'POST',body:JSON.stringify({challenge_id:cid,code})});
 showLog(b.log||b.error||(ok?'完成':'失败'));loadStatus();
}
function showLog(t){const p=document.getElementById('log');p.style.display='block';p.textContent=t;}
async function logout(){await j('/api/logout',{method:'POST'});sessionStorage.clear();location.href='/';}
loadStatus();
</script></body></html>"##;
