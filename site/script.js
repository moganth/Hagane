/* AUTO-GENERATED — do not edit by hand. Run: python site/generate_site.py */

function toggleSidebar() {
  document.getElementById('sidebar').classList.toggle('open');
}

document.getElementById('main-content').addEventListener('click', function() {
  document.getElementById('sidebar').classList.remove('open');
});

function toggleGroup(groupId) {
  var el = document.getElementById('group-' + groupId);
  if (el) el.classList.toggle('collapsed');
}

// Active link highlighting on scroll
var allAnchors = document.querySelectorAll('.section-anchor[id]');
var allNavLinks = document.querySelectorAll('.sidebar a.nav-link');

function updateActiveLink() {
  var scrollY = window.scrollY + 80;
  var current = '';
  allAnchors.forEach(function(a) { if (a.offsetTop <= scrollY) current = a.id; });
  allNavLinks.forEach(function(link) {
    var href = link.getAttribute('href');
    if (!href) return;
    var target = href.slice(1);
    link.classList.toggle('active', target === current);
  });
}

window.addEventListener('scroll', function() {}, { passive: true });
// Active link is managed by pagShow()

// ─── Pagination ───
var pagSections = Array.from(document.querySelectorAll('.doc-section'));
var pagCurrent = 0;

function pagTitles() {
  return pagSections.map(function(s) {
    var h = s.querySelector('h1') || s.querySelector('h2');
    return h ? h.textContent.trim() : '';
  });
}

function pagShow(idx, push) {
  if (idx < 0 || idx >= pagSections.length) return;
  pagSections[pagCurrent].classList.remove('active');
  pagCurrent = idx;
  pagSections[pagCurrent].classList.add('active');

  var titles = pagTitles();
  var prevBtn   = document.getElementById('pagPrev');
  var nextBtn   = document.getElementById('pagNext');
  var info      = document.getElementById('pagInfo');
  var prevTitle = document.getElementById('pagPrevTitle');
  var nextTitle = document.getElementById('pagNextTitle');

  prevBtn.disabled = pagCurrent === 0;
  nextBtn.disabled = pagCurrent === pagSections.length - 1;
  if (prevTitle) prevTitle.textContent = pagCurrent > 0 ? titles[pagCurrent - 1] : '';
  if (nextTitle) nextTitle.textContent = pagCurrent < pagSections.length - 1 ? titles[pagCurrent + 1] : '';
  if (info) info.textContent = (pagCurrent + 1) + ' · ' + pagSections.length;

  var sectionId = pagSections[pagCurrent].id;
  document.querySelectorAll('.sidebar a[href^="#"]').forEach(function(link) {
    link.classList.toggle('active', link.getAttribute('href') === '#' + sectionId);
  });

  window.scrollTo(0, 0);
  if (push) history.pushState(null, '', '#' + sectionId);
}

function pagGo(delta) { pagShow(pagCurrent + delta, true); }

(function() {
  if (!pagSections.length) return;
  var hash = window.location.hash.slice(1);
  var start = 0;
  if (hash) {
    for (var i = 0; i < pagSections.length; i++) {
      if (pagSections[i].id === hash) { start = i; break; }
    }
  }
  pagSections[start].classList.add('active');
  pagCurrent = start;
  pagShow(start, false);
})();

document.querySelectorAll('.sidebar a[href^="#"]').forEach(function(link) {
  link.addEventListener('click', function(e) {
    var target = this.getAttribute('href').slice(1);
    for (var i = 0; i < pagSections.length; i++) {
      if (pagSections[i].id === target) {
        e.preventDefault();
        pagShow(i, true);
        document.getElementById('sidebar').classList.remove('open');
        return;
      }
    }
  });
});

window.addEventListener('popstate', function() {
  var hash = window.location.hash.slice(1);
  for (var i = 0; i < pagSections.length; i++) {
    if (pagSections[i].id === hash) { pagShow(i, false); return; }
  }
});

// ─── Theme toggle ───
(function() {
  var saved = localStorage.getItem('hagane-theme');
  if (saved === 'light') applyLight();
})();
function applyLight() {
  document.body.classList.add('light');
  var moon = document.getElementById('themeIconMoon');
  var sun  = document.getElementById('themeIconSun');
  if (moon) moon.style.display = 'none';
  if (sun)  sun.style.display  = 'block';
}
function applyDark() {
  document.body.classList.remove('light');
  var moon = document.getElementById('themeIconMoon');
  var sun  = document.getElementById('themeIconSun');
  if (moon) moon.style.display = 'block';
  if (sun)  sun.style.display  = 'none';
}
function toggleTheme() {
  if (document.body.classList.contains('light')) {
    applyDark();
    localStorage.setItem('hagane-theme', 'dark');
  } else {
    applyLight();
    localStorage.setItem('hagane-theme', 'light');
  }
}

// ─── Download version picker ───
function toggleDownloadMenu() {
  var wrapper = document.getElementById('downloadWrapper');
  var isOpen = wrapper.classList.toggle('open');
  if (isOpen && !wrapper.dataset.loaded) {
    wrapper.dataset.loaded = '1';
    loadReleases();
  }
}
function loadReleases() {
  fetch('https://api.github.com/repos/moganth/Hagane/releases')
    .then(function(r) { return r.json(); })
    .then(function(releases) {
      var menu = document.getElementById('downloadMenu');
      var loading = document.getElementById('downloadLoading');
      if (loading) loading.remove();
      releases.forEach(function(rel) {
        var asset = (rel.assets || []).find(function(a) { return a.name === 'hagane-setup.exe'; });
        if (!asset) return;
        var item = document.createElement('a');
        item.className = 'download-menu-item';
        item.href = asset.browser_download_url;
        item.textContent = rel.tag_name + (rel.prerelease ? '  (pre-release)' : '');
        menu.appendChild(item);
      });
      if (!menu.querySelector('.download-menu-item')) {
        menu.innerHTML = '<span class="download-menu-loading">No releases found</span>';
      }
    })
    .catch(function() {
      var loading = document.getElementById('downloadLoading');
      if (loading) loading.textContent = 'Failed to load — check connection';
    });
}
document.addEventListener('click', function(e) {
  var wrapper = document.getElementById('downloadWrapper');
  if (wrapper && !wrapper.contains(e.target)) {
    wrapper.classList.remove('open');
  }
});
