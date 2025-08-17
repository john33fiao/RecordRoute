function showTextOverlay(url) {
    const overlay = document.getElementById('textOverlay');
    const content = document.getElementById('overlayContent');
    const download = document.getElementById('overlayDownload');
    const similar = document.getElementById('similarDocs');
    overlay.classList.add('show-flex');
    content.textContent = '로딩중...';
    download.href = url;
    if (similar) similar.innerHTML = '';
    fetch(url)
        .then(resp => resp.text())
        .then(text => {
            content.textContent = text;
        })
        .catch(() => {
            content.textContent = '파일을 불러오지 못했습니다.';
        });
}

function showEmbeddingOverlay(url) {
    const overlay = document.getElementById('textOverlay');
    const content = document.getElementById('overlayContent');
    const download = document.getElementById('overlayDownload');
    const similar = document.getElementById('similarDocs');
    overlay.classList.add('show-flex');
    content.textContent = '로딩중...';
    download.href = url;
    if (similar) similar.innerHTML = '<p>유사 문서를 불러오는 중...</p>';
    fetch(url)
        .then(resp => resp.text())
        .then(text => {
            content.textContent = text;
        })
        .catch(() => {
            content.textContent = '파일을 불러오지 못했습니다.';
        });
    const relPath = url.replace(/^\/download\//, '');
    fetch(`/similar?file=${encodeURIComponent(relPath)}`)
        .then(resp => resp.json())
        .then(data => {
            if (!similar) return;
            if (Array.isArray(data) && data.length > 0) {
                const header = document.createElement('h4');
                header.textContent = '유사 문서';
                const list = document.createElement('ul');
                data.forEach(item => {
                    const li = document.createElement('li');
                    const a = document.createElement('a');
                    a.href = item.link;
                    a.textContent = `${item.file} (유사도: ${item.score.toFixed(3)})`;
                    a.download = '';
                    li.appendChild(a);
                    list.appendChild(li);
                });
                similar.innerHTML = '';
                similar.appendChild(header);
                similar.appendChild(list);
            } else {
                similar.innerHTML = '<p>유사 문서가 없습니다.</p>';
            }
        })
        .catch(() => {
            if (similar) similar.innerHTML = '<p>유사 문서를 불러오지 못했습니다.</p>';
        });
}

document.getElementById('overlayClose').addEventListener('click', () => {
    document.getElementById('textOverlay').classList.remove('show-flex');
});

document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        const textOverlay = document.getElementById('textOverlay');
        if (textOverlay.classList.contains('show-flex')) {
            textOverlay.classList.remove('show-flex');
        }
        if (typeof summaryPopup !== 'undefined' && summaryPopup.classList.contains('show-flex') && typeof hideSummaryPopup === 'function') {
            hideSummaryPopup();
        }
        if (typeof sttConfirmPopup !== 'undefined' && sttConfirmPopup.classList.contains('show-flex') && typeof hideSttConfirmPopup === 'function') {
            hideSttConfirmPopup();
        }
    }
});
