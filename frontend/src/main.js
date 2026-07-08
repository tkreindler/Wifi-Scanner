import Chart from "chart.js/auto";
import fixture from "./ui-fixture.json";

// Channel sets per band; ticks are placed exactly on these channels.
const BANDS = [
  { id: "band2_4", label: "2.4 GHz", el: "spec24", chans: [1,2,3,4,5,6,7,8,9,10,11,12,13,14], f: (c) => c===14?2484:2407+c*5 },
  { id: "band5",   label: "5 GHz", el: "spec5",  chans: [36,40,44,48,52,56,60,64,100,104,108,112,116,120,124,128,132,136,140,144,149,153,157,161,165], f: (c)=>5000+c*5 },
  { id: "band6",   label: "6 GHz", el: "spec6",  chans: [1,5,9,13,21,37,53,69,85,101,117,133,149,165,181,197,213,229], f: (c)=>5950+c*5 },
];

async function scan() {
  const t = window.__TAURI__;
  if (t?.core?.invoke) {
    const r = await t.core.invoke("scan");
    return { interface: r.interface, aps: r.access_points, supportedBands: r.supported_bands, warning: r.warning, retryAfterPermissionChange: r.retry_after_permission_change };
  }
  return { interface: fixture.interface + " (fixture)", aps: fixture.access_points, supportedBands: fixture.supported_bands, warning: fixture.warning, retryAfterPermissionChange: fixture.retry_after_permission_change };
}

function hue(s){let h=0;for(const c of s)h=(h*31+c.charCodeAt(0))%360;return h;}
function dome(ap){const half=ap.channel_width_mhz/2,c=ap.center_freq_mhz||ap.frequency_mhz,peak=ap.rssi_dbm,floor=-100;return[{x:c-half,y:floor},{x:c-half*0.8,y:peak},{x:c+half*0.8,y:peak},{x:c+half,y:floor}];}

const charts={};
function renderBand(band,aps,supportedBands){
  const spec=document.getElementById(band.el).parentElement;
  if(!supportedBands.includes(band.id)) spec.dataset.note=`${band.label} is not supported by this Wi-Fi adapter`;
  else delete spec.dataset.note;
  const set=aps.filter(a=>a.band===band.id).sort((a,b)=>a.rssi_dbm-b.rssi_dbm);
  set.forEach(a=>{const c=a.center_freq_mhz||a.frequency_mhz,h=a.channel_width_mhz/2,lo=c-h,hi=c+h;
    a._dom=!set.some(o=>{const oc=o.center_freq_mhz||o.frequency_mhz,oh=o.channel_width_mhz/2;return o!==a&&o.rssi_dbm>a.rssi_dbm&&oc-oh<hi&&oc+oh>lo;});});
  charts[band.id]?.destroy();
  const ticks=band.chans.map(band.f);
  const chart=new Chart(document.getElementById(band.el),{
    type:"scatter",
    data:{datasets:set.map(a=>{const h=hue(a.ssid);return{ssid:a.ssid,peakX:a.center_freq_mhz||a.frequency_mhz,peakY:a.rssi_dbm,hue:h,dom:a._dom,
      data:dome(a),showLine:true,fill:false,tension:0,pointRadius:0,
      borderColor:c=>`hsl(${h} 70% ${a.is_dfs?78:62}% / ${c.chart.$hover==null||c.chart.$hover===c.datasetIndex?1:0.18})`,
      borderWidth:c=>c.chart.$hover===c.datasetIndex?3:1.4}})},
    options:{responsive:true,maintainAspectRatio:false,animation:false,layout:{padding:{top:18}},
      plugins:{legend:{display:false},tooltip:{enabled:false}},
      onHover:(e)=>{const x=e.x,y=e.y;let best=null,bd=1e9;chart.data.datasets.forEach((d,i)=>{const px=chart.scales.x.getPixelForValue(d.peakX),py=chart.scales.y.getPixelForValue(d.peakY);const dist=Math.hypot(px-x,py-y);if(dist<bd){bd=dist;best=i;}});const h=bd<70?best:null;if(chart.$hover!==h){chart.$hover=h;chart.update("none");}},
      scales:{x:{type:"linear",min:band.f(band.chans[0])-15,max:band.f(band.chans.at(-1))+15,
        afterBuildTicks:ax=>{ax.ticks=ticks.map(v=>({value:v}));},
        ticks:{color:"#7d8694",font:{size:10},callback:v=>band.chans.find(c=>Math.abs(band.f(c)-v)<1)??""},grid:{color:"#1d242c"}},
        y:{min:-100,max:-20,ticks:{color:"#7d8694",stepSize:20},grid:{color:"#1d242c"},title:{display:true,text:"dBm",color:"#7d8694"}}}},
    plugins:[hoverLabel]});
  chart.canvas.addEventListener("mouseleave",()=>{if(chart.$hover!=null){chart.$hover=null;chart.update("none");}});
  charts[band.id]=chart;
}
// Default: label networks that dominate their full width. Hover: emphasize one.
const hoverLabel={afterDatasetsDraw(c){const{ctx,scales}=c;ctx.save();ctx.font="600 12px system-ui";const used=[];
  const draw=(d,strong)=>{const x=scales.x.getPixelForValue(d.peakX),y=scales.y.getPixelForValue(d.peakY);
    if(used.some(u=>Math.abs(u.x-x)<55&&Math.abs(u.y-y)<14))return;used.push({x,y});
    const t=(d.ssid||"(hidden)")+(strong?`  ${d.peakY} dBm`:""),w=ctx.measureText(t).width+12;ctx.fillStyle=strong?"rgba(10,12,16,.85)":"rgba(10,12,16,.6)";
    ctx.beginPath();ctx.roundRect(x-w/2,y-26,w,18,5);ctx.fill();ctx.fillStyle=`hsl(${d.hue} 80% ${strong?78:70}%)`;ctx.textAlign="center";ctx.fillText(t,x,y-13);};
  if(c.$hover!=null){draw(c.data.datasets[c.$hover],true);}
  else{[...c.data.datasets].filter(d=>d.dom).sort((a,b)=>b.peakY-a.peakY).forEach(d=>draw(d,false));}
  ctx.restore();}};

function render({interface:iface,aps,supportedBands,warning,retryAfterPermissionChange}){
  supportedBands=supportedBands?.length?supportedBands:BANDS.map(b=>b.id);
  showNotice(warning||"");
  maybeRefreshAfterPermissionChange(retryAfterPermissionChange);
  document.getElementById("iface").textContent=iface;
  const nav=document.getElementById("bands");nav.innerHTML="";
  for(const b of BANDS){const n=aps.filter(a=>a.band===b.id).length;const c=document.createElement("span");
    const supported=supportedBands.includes(b.id);c.className=`chip${supported?"":" unsupported"}`;c.dataset.band=b.id;c.textContent=supported?`${b.label}: ${n}`:`${b.label}: unsupported`;nav.appendChild(c);}
  document.getElementById("rows").innerHTML=aps.slice().sort((a,b)=>b.rssi_dbm-a.rssi_dbm).map(a=>
    `<tr><td>${a.ssid}</td><td>${a.channel}</td><td>${a.channel_width_mhz}</td><td>${a.rssi_dbm}</td><td>${a.channel_utilization??""}</td><td>${bandLabel(a.band)}</td><td>${a.is_dfs?"✓":""}</td></tr>`).join("");
  BANDS.forEach(b=>renderBand(b,aps,supportedBands));
  document.body.dataset.ready="1";
}
function bandLabel(id){return BANDS.find(b=>b.id===id)?.label??"Unknown";}
function showNotice(message){const el=document.getElementById("notice");el.textContent=message;el.hidden=!message;}
let permissionRefreshTimer=null,permissionRefreshUntil=0;
function maybeRefreshAfterPermissionChange(retry){
  if(!retry){clearPermissionRefresh();return;}
  if(!window.__TAURI__?.core?.invoke||permissionRefreshTimer)return;
  if(!permissionRefreshUntil)permissionRefreshUntil=Date.now()+30000;
  permissionRefreshTimer=setTimeout(async()=>{
    permissionRefreshTimer=null;
    if(Date.now()>permissionRefreshUntil){permissionRefreshUntil=0;return;}
    try{render(await scan());}catch{}
  },1500);
}
function clearPermissionRefresh(){
  permissionRefreshUntil=0;
  if(permissionRefreshTimer){clearTimeout(permissionRefreshTimer);permissionRefreshTimer=null;}
}
const scanBtn=document.getElementById("scan");
async function requestPermissions(){await window.__TAURI__?.core?.invoke?.("request_permissions");}
async function runScan(){scanBtn.disabled=true;scanBtn.querySelector(".spin").hidden=false;
  try{render(await scan());}catch(e){showNotice(String(e));}finally{scanBtn.disabled=false;scanBtn.querySelector(".spin").hidden=true;}}
scanBtn.addEventListener("click",runScan);

requestPermissions().finally(()=>scan().then(render).catch(e=>showNotice(String(e))));
