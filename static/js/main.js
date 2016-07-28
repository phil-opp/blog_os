window.onload = function() {
  var container = document.querySelector('#toc');

  var toc = initTOC({
      selector: 'h2, h3',
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
