/* Injecte le sprite <symbol id="i-nom"> dans la page (fonctionne en file://). Usage : <svg class="ico"><use href="#i-home"/></svg> */
(function(){
  var s='<svg xmlns="http://www.w3.org/2000/svg" style="display:none">';
  for(var c in window.ICONS)for(var n in window.ICONS[c])s+='<symbol id="i-'+n+'" viewBox="0 0 24 24">'+window.ICONS[c][n]+'</symbol>';
  s+='</svg>';
  document.body.insertAdjacentHTML('afterbegin',s);
  /* <i data-i="home"></i> => svg */
  document.querySelectorAll('i[data-i]').forEach(function(e){
    var cls=(e.getAttribute('class')||'');
    e.outerHTML='<svg class="ico '+cls+'" aria-hidden="true"><use href="#i-'+e.dataset.i+'"/></svg>';
  });
})();
