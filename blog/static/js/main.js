window.onload = function() {
  var container = document.querySelector('#toc');

  if (container != null) {
    resize_toc(container);
  }
}

function resize_toc(container) {
  var containerHeight = container.clientHeight;

  var resize = function() {
    if (containerHeight > document.documentElement.clientHeight - 100) {
      container.classList.add('coarse');
    } else {
      container.classList.remove('coarse');
    }
  };
  resize();

  var resizeId;
  window.onresize = function() {
    clearTimeout(resizeId);
    resizeId = setTimeout(resize, 300);
  };
}
