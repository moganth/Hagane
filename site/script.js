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

window.addEventListener('scroll', updateActiveLink, { passive: true });
updateActiveLink();

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
