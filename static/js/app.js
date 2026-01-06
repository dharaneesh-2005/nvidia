// WebSocket connection
let ws;
let reconnectInterval;

function connect() {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;
    
    ws = new WebSocket(wsUrl);
    
    ws.onopen = () => {
        console.log('Connected');
        document.getElementById('status').textContent = 'Connected';
        document.getElementById('status').classList.add('connected');
        clearInterval(reconnectInterval);
    };
    
    ws.onmessage = (event) => {
        const data = JSON.parse(event.data);
        handleMessage(data);
    };
    
    ws.onclose = () => {
        console.log('Disconnected');
        document.getElementById('status').textContent = 'Disconnected';
        document.getElementById('status').classList.remove('connected');
        
        reconnectInterval = setInterval(() => {
            console.log('Reconnecting...');
            connect();
        }, 3000);
    };
    
    ws.onerror = (error) => {
        console.error('WebSocket error:', error);
    };
}

function handleMessage(data) {
    switch(data.type) {
        case 'transcription':
            const transcriptionEl = document.getElementById('transcription');
            transcriptionEl.textContent = data.text;
            transcriptionEl.scrollTop = transcriptionEl.scrollHeight;
            break;
            
        case 'answer':
            const answerEl = document.getElementById('answer');
            answerEl.textContent = data.text;
            answerEl.scrollTop = answerEl.scrollHeight;
            break;
            
        case 'screenshot':
            const img = document.getElementById('screenshot');
            const placeholder = document.getElementById('screenshot-placeholder');
            img.src = `data:image/png;base64,${data.image}`;
            img.style.display = 'block';
            placeholder.style.display = 'none';
            break;
            
        case 'analysis':
            const analysisEl = document.getElementById('analysis');
            analysisEl.textContent = data.text;
            analysisEl.scrollTop = analysisEl.scrollHeight;
            break;
    }
}

// Tab switching
document.querySelectorAll('.tab').forEach(tab => {
    tab.addEventListener('click', () => {
        const tabName = tab.dataset.tab;
        
        document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
        
        tab.classList.add('active');
        document.getElementById(`${tabName}-tab`).classList.add('active');
        
        if (tabName === 'code') {
            loadFileTree();
        }
    });
});

// Code tab functionality
async function loadFileTree() {
    try {
        const response = await fetch('/api/code/files');
        const data = await response.json();
        
        const fileList = document.getElementById('file-list');
        fileList.innerHTML = '';
        
        data.files.forEach(file => {
            const div = document.createElement('div');
            div.className = 'file-item';
            div.textContent = file.name;
            div.onclick = () => loadFileContent(file.path);
            fileList.appendChild(div);
        });
    } catch (error) {
        console.error('Error loading files:', error);
    }
}

async function loadFileContent(path) {
    try {
        const response = await fetch('/api/code/content', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path })
        });
        const data = await response.json();
        
        document.getElementById('current-file').textContent = path;
        document.getElementById('code-content').textContent = data.content;
    } catch (error) {
        console.error('Error loading file:', error);
    }
}

document.getElementById('code-query-btn').addEventListener('click', async () => {
    const query = document.getElementById('code-query-input').value;
    if (!query) return;
    
    document.getElementById('code-response').textContent = 'Thinking...';
    
    try {
        const response = await fetch('/api/code/query', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ query })
        });
        const data = await response.json();
        
        document.getElementById('code-response').textContent = data.response;
    } catch (error) {
        document.getElementById('code-response').textContent = 'Error: ' + error.message;
    }
});

// Enter key for code query
document.getElementById('code-query-input').addEventListener('keypress', (e) => {
    if (e.key === 'Enter') {
        document.getElementById('code-query-btn').click();
    }
});

// Initialize
connect();
