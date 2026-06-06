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
function rewrite(iframe){
if(iframe.hasAttribute("data-yao-tunnel"))return;
var src=iframe.getAttribute("src");
if(!src)return;
var port=needsTunnel(src);
if(!port)return;
iframe.removeAttribute("src");
iframe.setAttribute("data-yao-tunnel","1");
fetch("/__yao_desktop/tunnel",{
method:"POST",
headers:{"Content-Type":"application/json"},
body:JSON.stringify({port:port})
}).then(function(r){return r.json();}).then(function(d){
var u=new URL(src,location.href);
iframe.src="http://127.0.0.1:"+d.local_port+u.pathname+u.search;
}).catch(function(){
iframe.removeAttribute("data-yao-tunnel");
iframe.src=src;
});
}
var obs=new MutationObserver(function(muts){
muts.forEach(function(m){
m.addedNodes.forEach(function(n){
if(n.nodeType!==1)return;
if(n.tagName==="IFRAME")rewrite(n);
else if(n.querySelectorAll)n.querySelectorAll("iframe").forEach(rewrite);
});
});
});
function init(){
obs.observe(document.body||document.documentElement,{childList:true,subtree:true});
}
if(document.body)init();
else document.addEventListener("DOMContentLoaded",init);
})()
