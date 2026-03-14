const API_BASE = '/api/v1';
let ws = null;

async function fetchStatus() {
    try {
        const res = await fetch(`${API_BASE}/status`);
        const data = await res.json();
        document.getElementById('version').textContent = data.version || '-';
        document.getElementById('uptime').textContent = formatUptime(data.uptime_secs);
        document.getElementById('actors').textContent = data.actors_running || 0;
        document.getElementById('messages').textContent = data.messages_total || 0;
        updateStatusBadge(data.status);
    } catch (e) {
        console.error('Failed to fetch status:', e);
    }
}

async function fetchActors() {
    try {
        const res = await fetch(`${API_BASE}/actors`);
        const actors = await res.json();
        const tbody = document.querySelector('#actors-table tbody');
        tbody.innerHTML = actors.map(a => `
            <tr>
                <td>${a.id.substring(0, 8)}</td>
                <td>${a.name}</td>
                <td>${a.state}</td>
                <td>${a.messages}</td>
                <td>${a.errors}</td>
            </tr>
        `).join('');
    } catch (e) {
        console.error('Failed to fetch actors:', e);
    }
}

function formatUptime(secs) {
    if (!secs) return '-';
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

function updateStatusBadge(status) {
    const badge = document.getElementById('status-badge');
    badge.className = `status-${status || 'unknown'}`;
    badge.textContent = status || 'Unknown';
}

function connectWebSocket() {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${location.host}/ws`);
    ws.onmessage = (e) => {
        const msg = JSON.parse(e.data);
        if (msg.type === 'metrics_update') {
            document.getElementById('actors').textContent = msg.actors_running || 0;
            document.getElementById('messages').textContent = msg.messages_total || 0;
        } else if (msg.type === 'health_update') {
            updateStatusBadge(msg.status);
        }
    };
    ws.onclose = () => setTimeout(connectWebSocket, 5000);
}

document.addEventListener('DOMContentLoaded', () => {
    fetchStatus();
    fetchActors();
    connectWebSocket();
    setInterval(fetchStatus, 30000);
    setInterval(fetchActors, 30000);
});
