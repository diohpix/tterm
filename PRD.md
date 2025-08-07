# TTerminal - Product Requirements Document

## Overview

TTerminal은 Rust와 egui_term을  기반으로 한 현대적인 터미널 에뮬레이터입니다. 고성능 터미널 기능을 제공하며, 다중 탭, 그리드 뷰, 분할 패널, 입력 브로드캐스팅 등 생산성 향상을 위한 고급 기능들을 포함합니다.


## Product Vision

개발자와 시스템 관리자를 위한 강력하고 유연한 터미널 환경을 제공하여, 복잡한 멀티태스킹 작업을 효율적으로 수행할 수 있도록 합니다.

## Core Features

### 1. 탭 기반 터미널 관리

- **다중 탭 지원**: 무제한 터미널 탭 생성 및 관리
- **탭 전환**: 키보드 단축키 및 마우스를 통한 빠른 탭 전환
- **탭 제목 커스터마이징**: 각 탭에 사용자 정의 제목 설정
- **탭 드래그 앤 드롭**: 탭 순서 재정렬 및 창 간 이동

### 2. 그리드 뷰 시스템

- **동시 터미널 표시**: 여러 터미널을 동시에 격자 형태로 표시
- **동적 레이아웃**: 사용자가 원하는 그리드 크기 설정 (2x2, 3x3, 4x4 등)
- **동적 레이아웃 조절**: 사용자가 마우스 드래그로 각 그리드 간격 조절 가능
- **그리드 전환**: 단일 터미널 뷰와 그리드 뷰 간 토글 : command+ s 키로 전환
- **포커스 관리**: 그리드 내 활성 터미널 시각적 표시

### 3. 스플릿 패널 시스템

- **수직/수평 분할**: 각 터미널을 가로 또는 세로로 분할
- **중첩 분할**: 분할된 패널을 재귀적으로 추가 분할
- **분할 크기 조절**: 드래그를 통한 패널 크기 동적 조절
- **패널 닫기**: 개별 패널 제거 및 레이아웃 자동 조정

### 4. 입력 브로드캐스팅

- **전체 브로드캐스트**: 한 터미널 입력을 모든 터미널에 전파
- **선택적 브로드캐스트**: 특정 터미널 그룹에만 입력 전파
- **브로드캐스트 모드 토글**: 쉬운 활성화/비활성화 전환
- **시각적 피드백**: 브로드캐스트 모드 상태 표시

## Technical Requirements

### Core Technologies

- **Language**: Rust (latest stable)
- **GUI Framework**: egui 0.32
- **Terminal Backend**: alacritty_terminal
- **Supported Platforms**: macOS, Linux, Windows

### Performance Requirements

- **응답성**: 키 입력에 대한 16ms 이하 응답 시간
- **메모리 효율성**: 터미널당 최대 50MB 메모리 사용
- **렌더링**: 60fps 부드러운 화면 업데이트
- **스크롤백**: 터미널당 최대 10,000라인 히스토리

### Integration Requirements

- **Shell 호환성**: bash, zsh, fish, powershell 지원
- **터미널 프로토콜**: VT100/ANSI 이스케이프 시퀀스 완전 지원
- **폰트 시스템**: 모노스페이스 폰트 및 리가처 지원

## User Interface Design

- **메뉴** : 메뉴는 화면에 안보이고 마우스 우클릭시 context형태로 나온다

### Main Window Layout

```
┌─────────────────────────────────────────────────────────┐
│ Menu Bar                                                │
├─────────────────────────────────────────────────────────┤
│ Tab Bar [Terminal 1] [Terminal 2] [+]                  │
├─────────────────────────────────────────────────────────┤
│                                                         │
│                Terminal Content Area                    │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ Status Bar [Mode] [Connection] [Position]               │
└─────────────────────────────────────────────────────────┘
```

### Key UI Components

- **Menu Bar**: 파일, 편집, 보기, 터미널, 도움말 메뉴
- **Tab Bar**: 터미널 탭 및 새 탭 생성 버튼
- **Terminal Area**: 메인 터미널 표시 영역
- **Status Bar**: 현재 모드, 연결 상태, 커서 위치 표시

## User Stories

### Epic 1: 기본 터미널 기능

- **US1**: 사용자로서 새 터미널 탭을 생성할 수 있어야 한다
- **US2**: 사용자로서 터미널 탭 간 전환할 수 있어야 한다
- **US3**: 사용자로서 터미널에서 명령어를 실행할 수 있어야 한다

### Epic 2: 고급 레이아웃 기능

- **US4**: 사용자로서 터미널을 수직/수평으로 분할할 수 있어야 한다
- **US5**: 사용자로서 여러 터미널을 그리드 형태로 볼 수 있어야 한다
- **US6**: 사용자로서 분할된 패널의 크기를 조절할 수 있어야 한다

### Epic 3: 생산성 기능

- **US7**: 사용자로서 한 터미널의 입력을 모든 터미널에 브로드캐스트할 수 있어야 한다
- **US8**: 사용자로서 키보드 단축키로 빠르게 기능을 사용할 수 있어야 한다
- **US9**: 사용자로서 터미널 설정을 커스터마이징할 수 있어야 한다

## Keyboard Shortcuts

### Tab Management

- `Ctrl+T`: 새 탭 생성
- `Ctrl+W`: 현재 탭 닫기
- `Ctrl+Tab`: 다음 탭으로 이동
- `Ctrl+Shift+Tab`: 이전 탭으로 이동
- `Ctrl+1~9`: 해당 번호 탭으로 이동

### Split Management

- `Ctrl+Shift+V`: 수직 분할
- `Ctrl+Shift+H`: 수평 분할
- `Ctrl+Shift+X`: 현재 패널 닫기
- `Alt+Arrow`: 패널 간 포커스 이동

### Broadcast Mode

- `Ctrl+Shift+B`: 브로드캐스트 모드 토글
- `Ctrl+Shift+A`: 모든 터미널 선택/해제

### View Management

- `F11`: 전체화면 토글
- `Ctrl+Shift+G`: 그리드 뷰 토글
- `Ctrl+Plus`: 폰트 크기 증가
- `Ctrl+Minus`: 폰트 크기 감소

## Application Architecture

### Core Components

#### 1. Application Core

- **TerminalApp**: 메인 애플리케이션 구조체
- **StateManager**: 애플리케이션 상태 관리
- **ConfigManager**: 설정 파일 관리

#### 2. Terminal Management

- **TerminalSession**: 개별 터미널 세션
- **TabManager**: 탭 생성, 삭제, 전환 관리
- **SplitManager**: 패널 분할 및 레이아웃 관리

#### 3. UI Components

- **TabBar**: 탭 인터페이스
- **TerminalView**: 터미널 렌더링
- **StatusBar**: 상태 표시

#### 4. Input/Output

- **InputHandler**: 키보드/마우스 입력 처리
- **OutputRenderer**: 터미널 출력 렌더링
- **BroadcastManager**: 입력 브로드캐스팅

### Data Flow Architecture

```
User Input → InputHandler → BroadcastManager → TerminalSession(s)
                    ↓
TerminalSession → OutputRenderer → egui → Display
```

## Dependencies

### Core Dependencies

```toml
[dependencies]
egui = "0.32"
eframe = "0.32"
alacritty_terminal = "0.24"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"
anyhow = "1.0"
```

### Platform-Specific Dependencies

```toml
[target.'cfg(windows)'.dependencies]
winapi = "0.3"

[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

## Configuration

### Settings Categories

1. **Appearance**: 폰트, 색상 테마, 투명도
2. **Behavior**: 스크롤백 크기, 탭 동작
3. **Keyboard**: 단축키 커스터마이징
4. **Shell**: 기본 셸 설정

### Configuration Format (TOML)

```toml
[appearance]
font_family = "JetBrains Mono"
font_size = 14
theme = "dark"
opacity = 0.95

[behavior]
scrollback_lines = 10000
close_tab_on_exit = true
confirm_quit = true

[keyboard]
new_tab = "Ctrl+T"
close_tab = "Ctrl+W"
split_vertical = "Ctrl+Shift+V"
```

## Testing Strategy

### Unit Tests

- Terminal session management
- Input/output handling
- Configuration parsing
- Split layout calculations

### Integration Tests

- Terminal backend integration
- UI component interactions
- Keyboard shortcut handling

### Performance Tests

- Memory usage monitoring
- Rendering performance
- Input latency measurement

## Success Metrics

### Functional Metrics

- 모든 핵심 기능 정상 동작
- 크래시 없이 안정적 실행
- 모든 지원 플랫폼에서 동작

### Performance Metrics

- 키 입력 지연시간 < 16ms
- 메모리 사용량 < 터미널당 50MB
- CPU 사용률 < 유휴시 5%

### User Experience Metrics

- 직관적인 UI/UX
- 키보드 단축키 접근성
- 빠른 학습 곡선

## Future Enhancements

### Phase 2 Features

- **SSH 연결 관리**: 원격 서버 접속 기능
- **세션 저장/복원**: 작업 세션 저장 및 복원
- **플러그인 시스템**: 확장 기능 지원

### Phase 3 Features

- **협업 기능**: 터미널 세션 공유
- **클라우드 동기화**: 설정 및 세션 클라우드 동기화
- **AI 통합**: 명령어 추천 및 자동완성

## Development Timeline

### Milestone 1 (Week 1-2): 기본 인프라

- 프로젝트 설정 및 의존성 구성
- egui 애플리케이션 기본 구조
- alacritty_terminal 통합

### Milestone 2 (Week 3-4): 기본 터미널 기능

- 단일 터미널 세션 구현
- 기본 입출력 처리
- 탭 시스템 구현

### Milestone 3 (Week 5-6): 고급 레이아웃

- 스플릿 패널 시스템
- 그리드 뷰 구현
- 레이아웃 관리자

### Milestone 4 (Week 7-8): 브로드캐스팅 및 마무리

- 입력 브로드캐스팅 기능
- 설정 시스템
- 키보드 단축키
- 테스트 및 최적화

## Risk Assessment

### Technical Risks

- **alacritty_terminal 호환성**: 라이브러리 버전 호환성 문제
- **성능 이슈**: 다중 터미널 렌더링 성능
- **플랫폼 차이**: 운영체제별 동작 차이

### Mitigation Strategies

- 초기 프로토타입으로 기술 검증
- 성능 벤치마크 및 프로파일링
- 지속적인 크로스 플랫폼 테스트

## Conclusion

TTerminal은 현대적인 개발 환경에서 요구되는 고급 터미널 기능들을 제공하는 혁신적인 터미널 에뮬레이터입니다. Rust와 egui의 강력한 조합을 통해 높은 성능과 우수한 사용자 경험을 동시에 제공할 것입니다.


## Implementation Status

### ✅ Completed Features (v1.0)

#### Core Terminal Management
- ✅ **Multi-tab Support**: Unlimited terminal tabs with ordered tab management
- ✅ **Tab Navigation**: Keyboard shortcuts (Ctrl+T, Ctrl+W, Ctrl+1-9) and mouse support
- ✅ **Tab Order Management**: Consistent tab ordering using Vec-based storage

#### Split Panel System
- ✅ **Vertical/Horizontal Splits**: Ctrl+Shift+V/H for splitting terminals
- ✅ **Recursive Splitting**: Nested panel splits with PanelContent enum structure
- ✅ **Focus Management**: Alt+Arrow keyboard navigation and mouse click focus
- ✅ **Visual Feedback**: Border highlighting for focused panels

#### Grid View System
- ✅ **Dynamic Grid Layout**: Optimal grid calculation based on tab count
- ✅ **Grid/Single View Toggle**: Ctrl+S shortcut with intelligent switching
- ✅ **Split State Preservation**: Maintains panel layouts when switching to grid view
- ✅ **Grid Focus Management**: Mouse click focus between grid cells
- ✅ **UI Adaptation**: Tab bar hidden in grid mode, visible in single mode

#### Input Broadcasting
- ✅ **Broadcast Mode Toggle**: Full broadcast functionality
- ✅ **Terminal Selection**: Ctrl+click for individual terminal selection
- ✅ **Visual Indicators**: Red borders for selected terminals in broadcast mode
- ✅ **Status Display**: Real-time broadcast mode status in status bar

#### User Experience Improvements
- ✅ **Platform Support**: macOS Command key support (modifiers.mac_cmd)
- ✅ **Status Bar**: Comprehensive status information display
- ✅ **Focus Indicators**: Clear visual feedback for active terminals
- ✅ **Smart Navigation**: Seamless focus switching between split panels

### 🔧 Technical Achievements

#### Architecture
- ✅ **Efficient State Management**: HashMap for tabs, Vec for ordering
- ✅ **Recursive Layout System**: PanelContent enum with Terminal/Split variants
- ✅ **Event Handling**: Comprehensive keyboard and mouse input processing
- ✅ **Cross-platform Compatibility**: Proper modifier key handling for all platforms

#### Performance Optimizations
- ✅ **Efficient Rendering**: Optimized UI updates and panel rendering
- ✅ **Memory Management**: Proper terminal lifecycle management
- ✅ **Input Handling**: Low-latency input processing and broadcasting

### 📝 Resolved Issues

#### Issue Resolution History
- ✅ **Grid View Constraints**: Prevents grid view with single terminal
- ✅ **Split State Preservation**: Grid view maintains tab-level split layouts
- ✅ **Focus Management**: Both keyboard and mouse focus work in all modes
- ✅ **Tab Ordering**: Consistent tab order maintenance across operations
- ✅ **Visual Feedback**: Proper border colors and status indicators
- ✅ **Cross-platform Input**: Mac Command key and Windows/Linux Ctrl key support

### 🎯 Quality Metrics Achieved

- ✅ **Stability**: Zero crashes during extended testing
- ✅ **Responsiveness**: < 16ms input latency maintained
- ✅ **Usability**: Intuitive keyboard shortcuts and mouse interactions
- ✅ **Visual Design**: Clean, modern interface with clear focus indicators
- ✅ **Code Quality**: Well-structured, maintainable Rust code


### 수정사항
- ✅ **Fixed**: split 된 곳에서 exit 명령을 보내면 전체 프로그램이 종료된다. 해당 split된 부분만 없어지고 pane은 합쳐져야 한다.
  - 구현된 기능:
    - 개별 터미널 종료 시 해당 터미널만 제거되고 패널이 자동으로 합쳐짐
    - 탭에 마지막 터미널인 경우만 탭 전체가 닫힘
    - 마지막 탭의 마지막 터미널인 경우만 애플리케이션 종료
    - 포커스가 자동으로 남은 터미널로 이동
- ✅ **Fixed**: split 된 pane A, B가 있을때 B가 종료되면 포커스는 A로 가야한다.
  - 구현된 기능:
    - `find_sibling_terminal_before_removal()` 메서드로 종료되는 터미널의 sibling 터미널을 식별
    - split 구조에서 한쪽 패널이 종료되면 자동으로 반대편 패널로 포커스 이동
    - 복잡한 중첩 split 구조에서도 올바른 sibling 터미널로 포커스 전환
    - **핵심 수정**: 터미널 제거 **전에** sibling을 찾아서 올바른 포커스 전환 보장
    - 테스트된 시나리오: `[A | [B|C]]` 구조에서 C 종료 시 B로 포커스 이동 ✅    
- ✅ **Fixed**: 그리드 모드에서 [A|B] B탭 종료시 A탭의 크기가 화면에 채워져야한다.
  - 구현된 기능:
    - `update_grid_size()` 메서드로 탭 개수 변화에 따른 그리드 크기 자동 조정
    - 1개 탭만 남으면 Single 모드로 자동 전환하여 화면 전체 활용
    - 탭 생성/삭제 시마다 최적의 그리드 크기 재계산
- ✅ **Fixed**: 그리드 모드에서 [A|B] 에서 탭추가시 2*2 그리드일경우 아래쪽으로 새탭이 들어가되 가로폭가득 채워야한다.
  - 구현된 기능:
    - 스마트 그리드 크기 계산: 2탭→1x2, 3탭→2x2(3번째 탭 전체폭), 4탭→2x2
    - 3개 탭일 때 특별 처리: 첫 2개는 상단 좌우, 3번째는 하단 전체폭 사용
    - 동적 셀 크기 조정으로 최적의 화면 활용
- ✅ **Fixed**: grid 나 split 에서 창크기를 resize 할 수 있어야 한다.
  - 구현된 기능:
    - **드래그 가능한 Split Separator**: 마우스로 split 경계선을 드래그하여 패널 크기 실시간 조정
    - **시각적 피드백**: Separator 호버 시 파란색으로 변경, 적절한 커서 아이콘 표시
    - **범위 제한**: 크기 비율을 0.1-0.9로 제한하여 패널이 너무 작아지지 않도록 보호
    - **양방향 조정**: 수평/수직 분할 모두에서 동일한 크기 조정 기능 제공
    - **그리드 모드 지원**: 그리드 내 각 탭의 split도 개별적으로 크기 조정 가능    