let solutionModal = null;
let selectedSolutionIndex = 0;

document.addEventListener('DOMContentLoaded', () => {
    const container = document.querySelector('.container');
    const modalHTML = `
        <div class="solution-select-modal" id="solutionSelectModal">
            <div class="solution-select-content">
                <div class="solution-select-title">Get Detailed Solution</div>
                <div class="solution-select-options">
                    <div class="solution-option selected" data-option="detailed">📝 Brute + Optimal Solution</div>
                </div>
                <div class="solution-select-hint">↑↓ Navigate • Enter Select • Esc Cancel</div>
            </div>
        </div>
    `;
    container.insertAdjacentHTML('beforeend', modalHTML);
    solutionModal = document.getElementById('solutionSelectModal');
});

function showSolutionSelect() {
    if (solutionModal) {
        solutionModal.classList.add('active');
        selectedSolutionIndex = 0;
        updateSolutionSelection();
    }
}

function closeSolutionSelect() {
    if (solutionModal) {
        solutionModal.classList.remove('active');
    }
}

function updateSolutionSelection() {
    const options = document.querySelectorAll('.solution-option');
    options.forEach((opt, idx) => {
        opt.classList.toggle('selected', idx === selectedSolutionIndex);
    });
}

function navigateSolution(direction) {
    const options = document.querySelectorAll('.solution-option');
    if (direction === 'up') {
        selectedSolutionIndex = Math.max(0, selectedSolutionIndex - 1);
    } else if (direction === 'down') {
        selectedSolutionIndex = Math.min(options.length - 1, selectedSolutionIndex + 1);
    }
    updateSolutionSelection();
}

function selectSolution() {
    const options = document.querySelectorAll('.solution-option');
    const selected = options[selectedSolutionIndex];
    if (selected) {
        const option = selected.dataset.option;
        sendMessage(JSON.stringify({
            type: 'solution_select',
            option: option
        }));
        closeSolutionSelect();
    }
}
