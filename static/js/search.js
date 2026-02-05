// Manual question input
let searchModal = null;
let searchInput = null;

function initSearchModal() {
    console.log('Initializing search modal...');
    // Create modal HTML
    const modalHTML = `
        <div class="search-modal" id="searchModal">
            <div class="search-box">
                <input type="text" class="search-input" id="searchInput" placeholder="Type your question..." />
                <div class="search-hint">Press Enter to submit • Esc to close • Ctrl+Alt+S to toggle</div>
            </div>
        </div>
    `;
    document.body.insertAdjacentHTML('beforeend', modalHTML);
    
    searchModal = document.getElementById('searchModal');
    searchInput = document.getElementById('searchInput');
    console.log('Search modal initialized:', searchModal);
    
    // Close on background click
    searchModal.addEventListener('click', (e) => {
        if (e.target === searchModal) {
            closeSearchModal();
        }
    });
    
    // Handle Esc and Enter keys
    searchInput.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            closeSearchModal();
        } else if (e.key === 'Enter') {
            e.preventDefault();
            const question = searchInput.value.trim();
            if (question) {
                submitManualQuestion(question);
                searchInput.value = '';
                closeSearchModal();
            }
        }
    });

    const manualToggleBtn = document.getElementById('manualSearchToggle');
    if (manualToggleBtn) {
        manualToggleBtn.addEventListener('click', (e) => {
            e.preventDefault();
            toggleSearchModal();
        });
    }
    
    // Listen for Ctrl+Alt+S globally
    document.addEventListener('keydown', (e) => {
        if (e.ctrlKey && e.altKey && e.key.toLowerCase() === 's') {
            e.preventDefault();
            toggleSearchModal();
        }
    });
}

function toggleSearchModal() {
    if (!searchModal) {
        console.error('searchModal is null!');
        return;
    }
    if (searchModal.classList.contains('active')) {
        closeSearchModal();
    } else {
        openSearchModal();
    }
}

function openSearchModal() {
    console.log('openSearchModal called, searchModal:', searchModal);
    if (searchModal) {
        searchModal.classList.add('active');
        searchInput.focus();
        console.log('Modal opened');
    } else {
        console.error('searchModal is null!');
    }
}

function closeSearchModal() {
    if (searchModal) {
        searchModal.classList.remove('active');
        searchInput.value = '';
        // Notify backend to deactivate search mode
        if (window.sendMessage) {
            window.sendMessage('search_closed');
        } else if (ws && ws.readyState === WebSocket.OPEN) {
            ws.send('search_closed');
        }
    }
}

function submitManualQuestion(question) {
    console.log('🔍 [SEARCH LOG] Submitting manual question:', question);
    
    const messagePayload = {
        type: 'manual_question',
        text: question,
        auto_index: false
    };
    
    console.log('🔍 [SEARCH LOG] Message payload:', JSON.stringify(messagePayload, null, 2));
    
    if (window.sendMessage) {
        console.log('🔍 [SEARCH LOG] Sending via window.sendMessage');
        window.sendMessage(JSON.stringify(messagePayload));
    } else if (ws && ws.readyState === WebSocket.OPEN) {
        console.log('🔍 [SEARCH LOG] Sending via WebSocket');
        ws.send(JSON.stringify(messagePayload));
    } else {
        console.error('🔍 [SEARCH LOG] ERROR: No available connection method');
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', initSearchModal);
