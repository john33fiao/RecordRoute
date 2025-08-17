document.getElementById('searchBtn').addEventListener('click', async () => {
    const q = document.getElementById('searchInput').value.trim();
    if (!q) return;

    const searchBtn = document.getElementById('searchBtn');
    const originalText = searchBtn.textContent;

    try {
        searchBtn.textContent = '검색 중...';
        searchBtn.disabled = true;

        const resp = await fetch(`/search?q=${encodeURIComponent(q)}`);
        const data = await resp.json();

        const list = document.getElementById('searchResults');
        list.innerHTML = '';

        if (!resp.ok) {
            const errorMsg = data.error || '검색 중 오류가 발생했습니다.';
            list.innerHTML = `<li class="error-text">${errorMsg}</li>`;
            if (data.details) {
                console.error('검색 오류 상세:', data.details);
            }
        } else if (Array.isArray(data) && data.length === 0) {
            list.innerHTML = '<li class="text-muted">검색 결과가 없습니다.</li>';
        } else if (Array.isArray(data)) {
            data.forEach(item => {
                const li = document.createElement('li');
                const a = document.createElement('a');
                a.href = item.link;
                a.textContent = `${item.file} (유사도: ${item.score.toFixed(3)})`;
                a.target = '_blank';
                a.classList.add('link');
                li.appendChild(a);
                list.appendChild(li);
            });
        } else {
            list.innerHTML = '<li class="error-plain">예상치 못한 응답 형식입니다.</li>';
        }
    } catch (err) {
        const list = document.getElementById('searchResults');
        list.innerHTML = `<li class="error-text">네트워크 오류: ${err.message}</li>`;
        console.error('검색 네트워크 오류:', err);
    } finally {
        searchBtn.textContent = originalText;
        searchBtn.disabled = false;
    }
});
