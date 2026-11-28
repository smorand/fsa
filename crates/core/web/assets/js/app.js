(function(){
var D=document,R=D.documentElement,$=function(s,c){return(c||D).querySelector(s)},$$=function(s,c){return[].slice.call((c||D).querySelectorAll(s))};
var THEMES={light:'Light',dark:'Dark','carbon-light':'Carbon light','carbon-dark':'Carbon dark',green:'Green'};
function setTheme(t){R.setAttribute('data-theme',t);try{localStorage.setItem('pyr-theme',t)}catch(e){}$$('.proposal-bar button').forEach(function(b){b.classList.toggle('on',b.dataset.t===t)});$$('[data-ridge]').forEach(ridge)}
try{R.setAttribute('data-theme',localStorage.getItem('pyr-theme')||'light')}catch(e){}

/* Logo + ridge */
function logo(){return '<svg viewBox="0 0 48 48"><rect width="48" height="48" rx="12" fill="var(--primary)"/><path d="M5 37L18 14l8 12 5-7 12 18z" fill="#fff" opacity=".95"/><path d="M18 14l-4.5 8 3 2 2-2 2.5 2 2.5-2z" fill="var(--primary-soft)"/><circle cx="36" cy="12" r="3" fill="var(--accent)"/><path d="M18 14V5M15.5 8h5M16.5 10.5h3" stroke="#fff" stroke-width="1.8" stroke-linecap="round" fill="none"/><circle cx="18" cy="4.5" r="1.4" fill="var(--accent)"/></svg>'}
function ridge(el){
  var cs=getComputedStyle(R),c=function(n){return cs.getPropertyValue(n).trim()};
  el.setAttribute('viewBox','0 0 1200 200');el.setAttribute('preserveAspectRatio','none');
  el.innerHTML='<path d="M0 200V110l90-40 70 30 110-70 80 60 90-50 100 80 120-90 80 70 110-60 90 50 120-70 140 90V200z" fill="'+c('--hero-3')+'" opacity=".45"/>'+
  '<path d="M0 200V140l120-60 60 40 130-80 90 70 110-40 130 70 100-60 120 50 100-50 140 60 100-40V200z" fill="'+c('--hero-4')+'" opacity=".6"/>'+
  '<path d="M0 200V170l150-50 80 30 120-70 100 60 140-30 130 40 120-60 90 50 130-30 140 50V200z" fill="'+c('--hero-4')+'"/>';
}
D.addEventListener('DOMContentLoaded',function(){
  try{if(localStorage.getItem('pyr-side')==='0'&&$('.shell'))$('.shell').classList.add('collapsed')}catch(e){}
  $$('[data-logo]').forEach(function(e){e.innerHTML=logo()});
  /* Barre de proposition de thème */
  var bar=D.createElement('div');bar.className='proposal-bar';bar.innerHTML='<span>Proposition</span>'+Object.keys(THEMES).map(function(k){return '<button data-t="'+k+'">'+THEMES[k]+'</button>'}).join('');
  D.body.appendChild(bar);bar.addEventListener('click',function(e){if(e.target.dataset.t)setTheme(e.target.dataset.t)});
  setTheme(R.getAttribute('data-theme'));

  /* Tabs : .tabs[data-tabs] > .tab[data-tab=id] ; panels .panel#id */
  $$('[data-tabs]').forEach(function(t){t.addEventListener('click',function(e){var b=e.target.closest('.tab');if(!b)return;
    $$('.tab',t).forEach(function(x){x.classList.toggle('active',x===b)});
    var scope=D.getElementById(t.dataset.tabs)||D;$$('.panel',scope).forEach(function(p){p.classList.toggle('active',p.id===b.dataset.tab)})})});

  /* Dropdowns */
  D.addEventListener('click',function(e){
    var t=e.target.closest('[data-dd]');
    $$('.dd.open').forEach(function(d){if(!t||d!==t.closest('.dd'))d.classList.remove('open')});
    if(t){e.preventDefault();t.closest('.dd').classList.toggle('open');return}
    var o=e.target.closest('[data-open]');if(o){var m=D.getElementById(o.dataset.open);m&&m.classList.add('open');return}
    var c=e.target.closest('[data-close]');if(c){var ov=c.closest('.overlay');ov&&ov.classList.remove('open');return}
    if(e.target.classList.contains('overlay'))e.target.classList.remove('open');
    var sb=e.target.closest('[data-sidebar-toggle]');if(sb){var sh=$('.shell');if(sh){if(window.innerWidth<=900)sh.classList.toggle('open');else{sh.classList.toggle('collapsed');try{localStorage.setItem('pyr-side',sh.classList.contains('collapsed')?'0':'1')}catch(x){}}}return}
    var l=e.target.closest('[data-lang]');if(l){$$('.lang-item').forEach(function(x){x.classList.toggle('on',x===l)});$$('[data-lang-label]').forEach(function(x){x.textContent=l.dataset.lang.toUpperCase()});toast('Langue : '+l.textContent.trim().replace('✓','')) }
    var ts=e.target.closest('[data-toast]');if(ts)toast(ts.dataset.toast);
    var pw=e.target.closest('[data-togglepw]');if(pw){var i=pw.closest('.input-wrap').querySelector('input');i.type=i.type==='password'?'text':'password'}
  });
  D.addEventListener('keydown',function(e){if(e.key==='Escape')$$('.overlay.open').forEach(function(o){o.classList.remove('open')})});
  /* OTP auto-advance */
  $$('.otp').forEach(function(o){var ins=$$('input',o);ins.forEach(function(i,k){i.addEventListener('input',function(){if(i.value&&ins[k+1])ins[k+1].focus()});i.addEventListener('keydown',function(e){if(e.key==='Backspace'&&!i.value&&ins[k-1])ins[k-1].focus()})})});
  /* Charts */
  $$('[data-chart]').forEach(chart);
});
function toast(msg){var w=$('.toasts')||(function(){var d=D.createElement('div');d.className='toasts';D.body.appendChild(d);return d})();var t=D.createElement('div');t.className='toast';t.innerHTML='<svg class="ico"><use href="#i-check-circle"/></svg>'+msg;w.appendChild(t);setTimeout(function(){t.remove()},2800)}
window.toast=toast;

/* Mini charts SVG : data-chart="line|area|bar|donut|spark" data-values="1,2,3" data-values2="..." data-labels="a,b,c" */
function num(s){return s.split(',').map(Number)}
function chart(el){
  var type=el.dataset.chart,v=num(el.dataset.values),v2=el.dataset.values2?num(el.dataset.values2):null,labels=el.dataset.labels?el.dataset.labels.split(','):null;
  var W=el.dataset.w?+el.dataset.w:600,H=el.dataset.h?+el.dataset.h:220,p={l:34,r:10,t:10,b:24};
  var col=function(n){return 'var(--chart-'+n+')'};var s='';
  if(type==='donut'){var tot=v.reduce(function(a,b){return a+b},0),a=-Math.PI/2,R0=70,r0=44;s='<svg class="chart" viewBox="0 0 200 200" width="100%" style="max-width:200px">';
    v.forEach(function(x,i){var b=a+x/tot*2*Math.PI,L=b-a>Math.PI?1:0;
      s+='<path d="M'+(100+R0*Math.cos(a))+' '+(100+R0*Math.sin(a))+'A'+R0+' '+R0+' 0 '+L+' 1 '+(100+R0*Math.cos(b))+' '+(100+R0*Math.sin(b))+'L'+(100+r0*Math.cos(b))+' '+(100+r0*Math.sin(b))+'A'+r0+' '+r0+' 0 '+L+' 0 '+(100+r0*Math.cos(a))+' '+(100+r0*Math.sin(a))+'Z" fill="'+col(i%6+1)+'" stroke="var(--surface)" stroke-width="2"/>';a=b});
    s+='<text x="100" y="98" text-anchor="middle" style="font:600 20px var(--font-display);fill:var(--text)">'+tot+'</text><text x="100" y="114" text-anchor="middle">total</text></svg>';el.innerHTML=s;return}
  if(type==='spark'){W=120;H=36;p={l:2,r:2,t:3,b:3}}
  var all=v.concat(v2||[]),max=Math.max.apply(null,all)*1.1,min=type==='bar'?0:Math.min.apply(null,all)*.9;
  var iw=W-p.l-p.r,ih=H-p.t-p.b,n=v.length;
  var X=function(i){return p.l+(type==='bar'?(i+.5)/n:i/(n-1))*iw},Y=function(x){return p.t+ih-(x-min)/(max-min)*ih};
  s='<svg class="chart" viewBox="0 0 '+W+' '+H+'" width="100%" '+(type==='spark'?'style="max-width:120px"':'')+'>';
  if(type!=='spark'){s+='<g class="grid">';for(var g=0;g<=4;g++){var y=p.t+ih*g/4;s+='<line x1="'+p.l+'" x2="'+(W-p.r)+'" y1="'+y+'" y2="'+y+'"/><text x="'+(p.l-6)+'" y="'+(y+4)+'" text-anchor="end">'+Math.round(max-(max-min)*g/4)+'</text>'}s+='</g>';
    if(labels)labels.forEach(function(l,i){s+='<text x="'+X(i)+'" y="'+(H-6)+'" text-anchor="middle">'+l+'</text>'})}
  function line(arr,c,fill){var d=arr.map(function(x,i){return(i?'L':'M')+X(i)+' '+Y(x)}).join('');
    if(fill)s+='<path d="'+d+'L'+X(n-1)+' '+(p.t+ih)+'L'+X(0)+' '+(p.t+ih)+'Z" fill="'+c+'" opacity=".15"/>';
    s+='<path d="'+d+'" fill="none" stroke="'+c+'" stroke-width="2.2" stroke-linejoin="round" stroke-linecap="round"/>'}
  if(type==='bar'){var bw=iw/n*(v2?.36:.55);v.forEach(function(x,i){s+='<rect x="'+(X(i)-(v2?bw:bw/2))+'" y="'+Y(x)+'" width="'+bw+'" height="'+(p.t+ih-Y(x))+'" rx="4" fill="'+col(1)+'"/>';if(v2)s+='<rect x="'+X(i)+'" y="'+Y(v2[i])+'" width="'+bw+'" height="'+(p.t+ih-Y(v2[i]))+'" rx="4" fill="'+col(2)+'"/>'})}
  else{line(v,col(1),type==='area'||type==='spark');if(v2)line(v2,col(2),type==='area')}
  el.innerHTML=s+'</svg>';
}
})();
