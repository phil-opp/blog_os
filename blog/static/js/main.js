window.onload = function() {
  show_lang_selector();

  var container = document.querySelector('#toc-aside');

  if (container != null) {
    resize_toc(container);
    toc_scroll_position(container);
    window.onscroll = function() { toc_scroll_position(container) };
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

function toc_scroll_position(container) {
  if (container.offsetParent === null) {
    // skip computation if ToC is not visible
    return;
  }
  var items = container.querySelectorAll("li")

  // remove active class for all items
  for (item of container.querySelectorAll("li")) {
    item.classList.remove("active");
  }

  // look for active item
  var site_offset = document.documentElement.scrollTop;
  var current_toc_item = null;
  for (item of container.querySelectorAll("li")) {
    if (item.offsetParent === null) {
      // skip items that are not visible
      continue;
    }
    var anchor = item.firstElementChild.getAttribute("href");
    var heading = document.querySelector(anchor);
    if (heading.offsetTop <= (site_offset + document.documentElement.clientHeight / 3)) {
      current_toc_item = item;
    } else {
      break;
    }
  }

  // set active class for current ToC item
  if (current_toc_item != null) {
    current_toc_item.classList.add("active");
  }
}

function show_lang_selector() {
  var show_lang_selector = false;
  for (language_selector of document.querySelectorAll('#language-selector li')) {
    var lang = language_selector.getAttribute("data-lang-switch-to");
    this.console.log(lang)
    if (this.navigator.languages.includes(lang)) {
      this.console.log("supported!");
      language_selector.classList.remove("hidden");
      show_lang_selector = true
    }
  }
  if (show_lang_selector) {
    document.querySelector("#language-selector").classList.remove("hidden")
  }
}
