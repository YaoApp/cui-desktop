(function(){
var PROXY_PORT=__PROXY_PORT__;
var REMOTE_HOST="__REMOTE_HOST__";
var MAIN_PORT=__MAIN_PORT__;

function needsTunnel(src){
try{
var u=new URL(src,location.href);
var h=u.hostname;
var p=parseInt(u.port)||(u.protocol==="https:"?443:80);
if(h===REMOTE_HOST&&p!==MAIN_PORT)return p;
if((h==="127.0.0.1"||h==="localhost")&&p!==PROXY_PORT)return p;
return 0;
}catch(e){return 0;}
}

var origSetAttribute=Element.prototype.setAttribute;
var srcDesc=Object.getOwnPropertyDescriptor(HTMLIFrameElement.prototype,'src');
if(!srcDesc||!srcDesc.set){return;}

function realSetSrc(el,v){srcDesc.set.call(el,v);}

function showError(el,msg,detail){
origSetAttribute.call(el,"srcdoc",
'<html><body style="display:flex;align-items:center;justify-content:center;height:100%;margin:0;font-family:system-ui;color:#666"><div style="text-align:center"><p>'+msg+'</p><p style="font-size:12px;opacity:0.6">'+detail+'</p></div></body></html>');
}

function doTunnel(el,src,port){
origSetAttribute.call(el,"data-yao-tunnel","pending");
var timer=setTimeout(function(){
origSetAttribute.call(el,"data-yao-tunnel","timeout");
showError(el,"Tunnel creation timed out","Failed to connect to remote port "+port);
},8000);
fetch("/__yao_desktop/tunnel",{
method:"POST",
headers:{"Content-Type":"application/json"},
body:JSON.stringify({port:port})
}).then(function(r){return r.json();}).then(function(d){
clearTimeout(timer);
if(d.local_port){
origSetAttribute.call(el,"data-yao-tunnel","ready");
var u=new URL(src,location.href);
realSetSrc(el,"http://127.0.0.1:"+d.local_port+u.pathname+u.search);
}else{
origSetAttribute.call(el,"data-yao-tunnel","error");
showError(el,"Tunnel creation failed","Port "+port+" unavailable");
}
}).catch(function(){
clearTimeout(timer);
origSetAttribute.call(el,"data-yao-tunnel","error");
showError(el,"Tunnel creation failed","Port "+port+" unavailable");
});
}

function intercept(el,value){
var src=String(value);
var port=needsTunnel(src);
if(!port)return false;
var state=el.getAttribute("data-yao-tunnel");
if(state==="pending")return true;
if(state==="ready"&&el.getAttribute("data-yao-tunnel-src")===src)return true;
origSetAttribute.call(el,"data-yao-tunnel-src",src);
doTunnel(el,src,port);
return true;
}

Object.defineProperty(HTMLIFrameElement.prototype,'src',{
set:function(v){if(!intercept(this,v))srcDesc.set.call(this,v);},
get:srcDesc.get,
configurable:true,enumerable:true
});

HTMLIFrameElement.prototype.setAttribute=function(name,value){
if(name==="src"&&intercept(this,value))return;
return origSetAttribute.call(this,name,value);
};

var obs=new MutationObserver(function(muts){
muts.forEach(function(m){
m.addedNodes.forEach(function(n){
if(n.nodeType!==1)return;
var frames=[];
if(n.tagName==="IFRAME")frames.push(n);
else if(n.querySelectorAll){
var found=n.querySelectorAll("iframe");
for(var i=0;i<found.length;i++)frames.push(found[i]);
}
for(var i=0;i<frames.length;i++){
var f=frames[i];
if(f.getAttribute("data-yao-tunnel"))continue;
var src=f.getAttribute("src");
if(!src)continue;
var port=needsTunnel(src);
if(!port)continue;
f.removeAttribute("src");
doTunnel(f,src,port);
}
});
});
});
function init(){obs.observe(document.body||document.documentElement,{childList:true,subtree:true});}
if(document.body)init();
else document.addEventListener("DOMContentLoaded",init);
})()
