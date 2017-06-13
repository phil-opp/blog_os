window.onload = function() {
  var container = document.querySelector('#toc');

  if (container != null) {
    var toc = initTOC({
        selector: 'h2, h3',
        scope: 'main',
        overwrite: false,
        prefix: 'toc'
    });

    toc.innerHTML = toc.innerHTML.split("🔗\n").join("");

    var heading = document.createElement("H2");
    var heading_text = document.createTextNode("Table of Contents");
    heading.appendChild(heading_text);

    container.appendChild(heading);
    container.appendChild(toc);

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
