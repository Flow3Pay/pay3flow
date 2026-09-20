"use client";

import styles from "./header.module.css";

const moonIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACWklEQVR4AbTVTYhNYRzH8TPCRs0kLxsWIvIywkrykvISTZRkbGiSQsqCIkuWyobkpZkiL5PQ2Eyk2BBGErOaZCMz0giRrEyNz/90z3XvzL23e6Zm+n3v/zz/5/n/f8957plzJyTj/FfTYHh4eDM60Il5Y9lLRQPN5uOyho+wH7GuQ8ytKCwr0ni7xHMcxGOsxW68QW6VGWi+TofrmIGzDQ0Nm8QpCL2Oj7wUDTRvVnwVTRqHjrsOLYoPvEJuFQ1U7sFc7ESplhoMcvwo5lZqYPcTVR5Ap0ZdYqmmG/RjTEoNVLZhKi5hpOILXzAyWe84M8jO+VOFwndyje4yW2NYvzKD2P0fx1PJoLfQLp6owmX9odRgoFIZ00H5CzjnLhaKuVRq8LdaJZMj5n7hIXIpM4jCZjuMx7Rag30m5lgzgOWu61Jm8KywuqUQRwV3cV9yI2bhLZMTmO26plIDxS+t+ow1qCrrnpichms4g34mvWjHqRLWm0uVGqRXSXJHbLWo6l2YT5j8QBzXLuMw+SbGy/CwuA0Rb4upigaKjsnE+6aLSfxnG1aX9fdwEhvQiJlW78VX9CFV0SAdJckWcTKeIpdsarGCOIUl4kWkKjOwi5+yq7FKwW/EC9CwtqzbakXWvFWfu8apygwiY/KFOAm3cENxfHktYhyB1H/JrcRNmQdoQllz42SUQSSZDOGQ66BV7MagZu/Rgw/4LteDZYjfjhVqijuXS1XRIJ3xoeAK4myDeJ1Hwy+mwvC0GE9NNI5fv3iapMpV0yBbyqQP7WjDDhzFeXRjKFtXKf4DAAD///Lx6McAAAAGSURBVAMASQPNMX2ya7kAAAAASUVORK5CYII=";
const sunIcon = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAACD0lEQVR4AbSTPS8FQRSG92o0+Ae0EkKDhk58NJQUoiASlUoQKolCoySESCQaCdEp0BGREBQK4W+goJDreSdnstmdXXe3cHOenDNn3nfO7t7duuiff6UGVKvVWVHmmkoNKHOw15YaUKlU9oQ3F8nBAB7BBSxlmem3ipy9ZfbO03vBAARX8AkuMOm535DfabwK1aDeLGsf8lz7hc/BAB7BOmxLwCEP5F1ogn2YMFSrt2uaCM8WrLOfiGCA38V4TN0Fl9CLeR6OjHn1QHtdpmUZRuYADCNIx0BXNcyhun2WcagHw3S2YMw8lMnIHIBkBZ5gAWqFNNLKE2jzBnSivOcKv8l/hmnuEclDSoYbwO3pTVnUFnUbuQHuoGhI22DeiLwI7g1zA1Kn/Ni65tWbTslrvVc9hxvAbeoL3VCH+o38AT1QNKT9MK9e2Q3qPZndABUpnlkPcJt6VJT5YZoBFPKQkpE34BBZB5xCrZBGWnkCbeYAu71H1ENc4SbUUydCPdikOQSP5qFMRuYASTB0k09gDm45bAemjB31QHsnpmUZRjCAA1ZgWlKM4+RR+IJJODBUqzdqGr2aM/iW2U9EMIDdfmgGFxxwBn3QSKNdqAb1zlj7aKGQlxRHMADjIKzFkrii/yLiTlzRXwX9H3GTKhhALzd4BPri3ReaK0ptlBqQ8hZalhrAI9AX777QQqdHUfQLAAD//91ClIsAAAAGSURBVAMAR3zSMQ+aPXkAAAAASUVORK5CYII=";

interface HeaderProps {
  brandHref?: string;
}

export function Header({
  brandHref = "/",
}: HeaderProps) {
  const toggleTheme = () => {
    const root = document.documentElement;
    const nextTheme = root.dataset.theme === "dark" ? "light" : "dark";
    root.dataset.theme = nextTheme;
    localStorage.setItem("pay3flow-theme", nextTheme);
  };

  return (
    <header className={styles.header}>
      <div className={styles.inner}>
        <a href={brandHref} className={styles.brand} aria-label="Pay3Flow home">
          <span className={styles.logo} aria-hidden="true">
            <svg width="26" height="26" viewBox="0 0 26 26" fill="none">
              <path d="M4 7.25 13 2l9 5.25v11.5L13 24l-9-5.25V7.25Z" fill="currentColor" />
              <path d="m8.2 10.2 4.8-2.8 4.8 2.8-4.8 2.8-4.8-2.8Zm0 5.3 4.8 2.8 4.8-2.8" stroke="#171a17" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </span>
          <span className={styles.wordmark}>Pay3Flow</span>
          <span className={styles.beta}>Beta</span>
        </a>

        <div className={styles.actions}>
          <span className={styles.marketStatus}>
            <span className={styles.pulse} />
            Markets live
          </span>
          <button
            className={styles.themeToggle}
            type="button"
            onClick={toggleTheme}
            aria-label="Переключить цветовую тему"
            title="Переключить тему"
          >
            <img className={styles.moonIcon} src={moonIcon} alt="" />
            <img className={styles.sunIcon} src={sunIcon} alt="" />
          </button>
          <a
            className={styles.githubLink}
            href="https://github.com/Flow3Pay/pay3flow"
            target="_blank"
            rel="noreferrer noopener"
            aria-label="Open Pay3Flow on GitHub"
          >
            <img
              src="https://cdn-icons-png.flaticon.com/512/2111/2111432.png"
              alt=""
            />
          </a>
        </div>
      </div>
    </header>
  );
}
