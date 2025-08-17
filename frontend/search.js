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
            list.innerHTML = `<li style="color: #dc3545; font-weight: bold;">${errorMsg}</li>`;
            if (data.details) {
                console.error('검색 오류 상세:', data.details);
            }
        } else if (Array.isArray(data) && data.length === 0) {
            list.innerHTML = '<li style="color: #6c757d; font-style: italic;">검색 결과가 없습니다.</li>';
        } else if (Array.isArray(data)) {
            data.forEach(item => {
                const li = document.createElement('li');
                const a = document.createElement('a');
                a.href = item.link;
                a.textContent = `${item.file} (유사도: ${item.score.toFixed(3)})`;
                a.target = '_blank';
                a.style.textDecoration = 'none';
                a.style.color = '#007bff';
                a.onmouseover = () => a.style.textDecoration = 'underline';
                a.onmouseout = () => a.style.textDecoration = 'none';
                li.appendChild(a);
                list.appendChild(li);
            });
        } else {
            list.innerHTML = '<li style="color: #dc3545;">예상치 못한 응답 형식입니다.</li>';
        }
    } catch (err) {
        const list = document.getElementById('searchResults');
        list.innerHTML = `<li style="color: #dc3545; font-weight: bold;">네트워크 오류: ${err.message}</li>`;
        console.error('검색 네트워크 오류:', err);
    } finally {
        searchBtn.textContent = originalText;
        searchBtn.disabled = false;
    }
});
