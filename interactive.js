document.addEventListener('DOMContentLoaded', () => {
    const svgObject = document.getElementById('architecture-svg');

    svgObject.addEventListener('load', () => {
        const svgDoc = svgObject.contentDocument;
        if (!svgDoc) {
            console.error("Could not access SVG document. Make sure it's from the same origin.");
            return;
        }

        const components = document.querySelectorAll('.component');

        components.forEach(component => {
            const componentId = component.dataset.componentId;
            // Mermaid.js generates IDs like `A-codegraph-core`
            const node = svgDoc.querySelector(`[id^="${componentId}-"]`);

            if (node) {
                component.addEventListener('mouseover', () => {
                    node.classList.add('highlight');
                });

                component.addEventListener('mouseout', () => {
                    node.classList.remove('highlight');
                });
            }
        });
    });

    // Handle case where SVG fails to load
    svgObject.addEventListener('error', () => {
        console.error('SVG file could not be loaded. Please ensure architecture.svg exists.');
    });
});