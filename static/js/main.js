window.onload = function() {
  var container = document.querySelector('#toc');

  var selector = "h2";
  if (container.className.split(" ").indexOf("coarse") == -1) {
    selector += ",h3";
  }

  var toc = initTOC({
      selector: selector,
      scope: '.post',
      overwrite: false,
      prefix: 'toc'
  });

  var heading = document.createElement("H2");
  var heading_text = document.createTextNode("Table of Contents");
  heading.appendChild(heading_text);

  container.appendChild(heading);
  container.appendChild(toc);
}
